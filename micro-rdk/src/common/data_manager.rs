use std::sync::Arc;
use std::time::Duration;

use crate::common::data_collector::{DataCollectionError, DataCollector};
use crate::common::data_store::DataStore;
use crate::google::protobuf::value::Kind;
use crate::proto::app::data_sync::v1::{DataCaptureUploadRequest, DataType, SensorData, UploadMetadata};
use crate::proto::app::v1::ConfigResponse;

use super::app_client::{AppClient, AppClientError};
use super::data_collector::ResourceMethodKey;
use super::data_store::{DataStoreError, WriteMode};
use super::robot::{LocalRobot, RobotError};
use async_io::Timer;
use bytes::BytesMut;
use futures_util::lock::Mutex;
use prost::{DecodeError, Message};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DataManagerError {
    #[error("no data collectors in manager")]
    NoCollectors,
    #[error(transparent)]
    CollectionError(#[from] DataCollectionError),
    #[error(transparent)]
    StoreError(#[from] DataStoreError),
    #[error("queried time interval {0} is not a multiple of minimum time_interval{1}")]
    ImproperTimeInterval(u64, u64),
    #[error("data service config does not exist or is improperly configured")]
    ConfigError,
    #[error("multiple data manager configurations detected")]
    MultipleConfigError,
    #[error(transparent)]
    InitializationRobotError(#[from] RobotError),
    #[error(transparent)]
    ProtoDecodeError(#[from] DecodeError),
    #[error(transparent)]
    AppClientError(#[from] AppClientError)
}

fn get_data_sync_interval(cfg: &ConfigResponse) -> Result<Option<Duration>, DataManagerError> {
    let robot_config = cfg.config.clone().ok_or(DataManagerError::ConfigError)?;
    let num_configs_detected = robot_config
        .services
        .iter()
        .filter(|svc_cfg| svc_cfg.r#type == *"data_manager")
        .count();
    if num_configs_detected > 1 {
        return Err(DataManagerError::MultipleConfigError);
    }
    Ok(
        if let Some(data_cfg) = robot_config
            .services
            .iter()
            .find(|svc_cfg| svc_cfg.r#type == *"data_manager")
        {
            let attrs = data_cfg
                .attributes
                .clone()
                .ok_or(DataManagerError::ConfigError)?;
            let sync_interval_mins = attrs
                .fields
                .get("sync_interval_mins")
                .ok_or(DataManagerError::ConfigError)?;
            if let Some(Kind::NumberValue(sync_interval_mins)) = sync_interval_mins.kind {
                Some(Duration::from_secs((sync_interval_mins * 60.0) as u64))
            } else {
                return Err(DataManagerError::ConfigError);
            }
        } else {
            None
        },
    )
}

fn read_from_store(store: &mut dyn DataStore, collector_key: &ResourceMethodKey) -> Result<Vec<BytesMut>, DataManagerError> {
    let mut readings_to_upload: Vec<BytesMut> = vec![];
    loop {
        match store.read_next_message(collector_key) {
            Ok(msg) => {
                if msg.is_empty() {
                    break;
                }
                readings_to_upload.push(msg);
            }
            Err(err) => return Err(err.into()),
        };
    }
    Ok(readings_to_upload)
}

pub struct DataManager<'a, StoreType> {
    collectors: Vec<DataCollector>,
    store: StoreType,
    sync_interval: Duration,
    min_interval: Duration,
    part_id: String,
    app_client: Arc<Mutex<Option<AppClient<'a>>>>
}

impl<'a, StoreType> DataManager<'a, StoreType>
where
    StoreType: DataStore,
{
    pub fn new(
        collectors: Vec<DataCollector>,
        store: StoreType,
        sync_interval: Duration,
        part_id: String,
        app_client: Arc<Mutex<Option<AppClient<'a>>>>
    ) -> Result<Self, DataManagerError> {
        let intervals = collectors.iter().map(|x| x.time_interval());
        let min_interval = intervals.min().ok_or(DataManagerError::NoCollectors)?;
        Ok(Self {
            collectors,
            store,
            sync_interval,
            min_interval,
            part_id,
            app_client
        })
    }

    pub fn from_robot_and_config(
        cfg: &ConfigResponse,
        part_id: String,
        robot: Arc<std::sync::Mutex<LocalRobot>>,
        app_client: Arc<Mutex<Option<AppClient<'a>>>>
    ) -> Result<Option<Self>, DataManagerError> {
        let sync_interval = get_data_sync_interval(cfg)?;
        if let Some(sync_interval) = sync_interval {
            let collectors = robot.lock().unwrap().data_collectors()?;
            let collector_keys: Vec<ResourceMethodKey> =
                collectors.iter().map(|c| c.resource_method_key()).collect();
            let store = StoreType::from_resource_method_keys(collector_keys)?;
            let data_manager_svc = DataManager::new(collectors, store, sync_interval, part_id, app_client)?;
            Ok(Some(data_manager_svc))
        } else {
            Ok(None)
        }
    }

    pub fn sync_interval_ms(&self) -> u64 {
        self.sync_interval.as_millis() as u64
    }

    pub fn min_interval_ms(&self) -> u64 {
        self.min_interval.as_millis() as u64
    }

    pub fn part_id(&self) -> String {
        self.part_id.clone()
    }

    pub(crate) fn collection_intervals(&self) -> Vec<u64> {
        let mut intervals: Vec<u64> = self
            .collectors
            .iter()
            .map(|x| {
                (x.time_interval().as_millis() as u64 / self.min_interval_ms())
                    * self.min_interval_ms()
            })
            .collect();
        intervals.sort();
        intervals.dedup();
        intervals
    }

    pub async fn run(&mut self) -> Result<(), DataManagerError> {
        let mut loop_counter: u64 = 0;
        loop {
            self.run_inner(loop_counter, false).await?;
            loop_counter += 1;
            Timer::after(self.min_interval).await;
        }
    }

    async fn run_inner(&mut self, loop_counter: u64, force_read: bool) -> Result<(), DataManagerError> {
        let min_interval_ms = self.min_interval_ms();
        if (loop_counter % (self.sync_interval_ms() / min_interval_ms)) == 0 && (loop_counter != 0)
        {
            if let Err(err) = self.sync(force_read).await {
                log::error!("error uploading data: {:?}", err);
            }
        }
        for interval in self.collection_intervals() {
            println!("here");
            if loop_counter % (interval / min_interval_ms) == 0 {
                self.collect_and_store_readings(interval)?;
            }
        }
        Ok(())
    }

    async fn sync(&mut self, force_read: bool) -> Result<(), DataManagerError> {
        let collectors = &self.collectors;
        for collector_key in collectors.iter().map(|c| c.resource_method_key()) {
            match self.app_client.lock().await.as_mut() {
                Some(app_client) => {
                    let readings_to_upload = read_from_store(&mut self.store, &collector_key)?;
                    let mut upload_request = DataCaptureUploadRequest {
                        metadata: Some(UploadMetadata {
                            part_id: self.part_id(),
                            component_type: collector_key.component_type,
                            r#type: DataType::TabularSensor.into(),
                            component_name: collector_key.r_name,
                            method_name: collector_key.method.to_string(),
                            ..Default::default()
                        }),
                        sensor_contents: vec![]
                    };
                    let data: Result<Vec<SensorData>, DecodeError> = readings_to_upload.into_iter().map(SensorData::decode).collect();
                    upload_request.sensor_contents = data?;
                    app_client.upload_data(upload_request).await?;
                },
                None => {
                    if force_read {
                        let _ = read_from_store(&mut self.store, &collector_key)?;
                    }
                    log::info!("app client unavailable for data sync, most likely because signaling dropped")
                }
            };
        }
        Ok(())
    }

    fn collect_and_store_readings(
        &mut self,
        time_interval_ms: u64,
    ) -> Result<(), DataManagerError> {
        for (collector_key, reading) in self.collect_readings_for_interval(time_interval_ms)? {
            self.store
                .write_message(&collector_key, reading, WriteMode::OverwriteOldest)?
        }
        Ok(())
    }

    // Here, time_interval_ms is required to be a multiple of the minimum time_interval among the collectors.
    // This function then collects readings from collectors whose time_interval is greater than time_interval_ms but
    // less than the next largest multiple of self.min_interval_ms
    fn collect_readings_for_interval(
        &mut self,
        time_interval_ms: u64,
    ) -> Result<Vec<(ResourceMethodKey, SensorData)>, DataManagerError> {
        let min_interval_ms = self.min_interval_ms();
        if time_interval_ms % min_interval_ms != 0 {
            return Err(DataManagerError::ImproperTimeInterval(
                time_interval_ms,
                min_interval_ms,
            ));
        }
        self.collectors
            .iter_mut()
            .filter(|coll| {
                (coll.time_interval().as_millis() as u64 / min_interval_ms)
                    == (time_interval_ms / min_interval_ms)
            })
            .map(|coll| Ok((coll.resource_method_key(), coll.call_method()?)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::mem::MaybeUninit;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use bytes::{BufMut, BytesMut};
    // use futures_util::lock::Mutex as AsyncMutex;
    use ringbuf::{LocalRb, Rb};

    use super::DataManager;
    use crate::common::data_store::WriteMode;
    use crate::common::encoder::EncoderError;
    use crate::common::{
        data_collector::{CollectionMethod, DataCollector, ResourceMethodKey},
        data_store::{DataStore, DataStoreError},
        robot::ResourceType,
        sensor::{
            GenericReadingsResult, Readings, Sensor, SensorError, SensorResult, SensorT,
            TypedReadingsResult,
        },
        status::{Status, StatusError},
    };
    use crate::google::protobuf::value::Kind;
    use crate::google::protobuf::Struct;
    use crate::proto::app::data_sync::v1::{sensor_data::Data, SensorData};

    #[derive(DoCommand)]
    struct TestSensorFailure {}

    impl Sensor for TestSensorFailure {}

    impl Readings for TestSensorFailure {
        fn get_generic_readings(&mut self) -> Result<GenericReadingsResult, SensorError> {
            Err(SensorError::SensorMethodUnimplemented(
                "test sensor failure",
            ))
        }
    }

    impl Status for TestSensorFailure {
        fn get_status(&self) -> Result<Option<Struct>, StatusError> {
            Err(StatusError::EncoderError(
                EncoderError::EncoderMethodUnimplemented,
            ))
        }
    }

    #[derive(DoCommand)]
    struct TestSensor {}

    impl Sensor for TestSensor {}

    impl Readings for TestSensor {
        fn get_generic_readings(&mut self) -> Result<GenericReadingsResult, SensorError> {
            Ok(self
                .get_readings()?
                .into_iter()
                .map(|v| (v.0, SensorResult::<f64> { value: v.1 }.into()))
                .collect())
        }
    }

    impl SensorT<f64> for TestSensor {
        fn get_readings(&self) -> Result<TypedReadingsResult<f64>, SensorError> {
            let mut x = HashMap::new();
            x.insert("thing".to_string(), 42.42);
            Ok(x)
        }
    }

    impl Status for TestSensor {
        fn get_status(&self) -> Result<Option<Struct>, StatusError> {
            Err(StatusError::EncoderError(
                EncoderError::EncoderMethodUnimplemented,
            ))
        }
    }

    struct NoOpStore {}

    impl DataStore for NoOpStore {
        fn read_next_message(
            &mut self,
            _collector_key: &ResourceMethodKey,
        ) -> Result<bytes::BytesMut, DataStoreError> {
            Err(DataStoreError::Unimplemented)
        }
        fn write_message(
            &mut self,
            _collector_key: &ResourceMethodKey,
            _message: SensorData,
            _write_mode: WriteMode,
        ) -> Result<(), DataStoreError> {
            Err(DataStoreError::Unimplemented)
        }
        fn from_resource_method_keys(
            _collector_keys: Vec<ResourceMethodKey>,
        ) -> Result<Self, DataStoreError> {
            Ok(Self {})
        }
    }

    #[test_log::test]
    fn test_collection_intervals() {
        let resource_1 = ResourceType::Sensor(Arc::new(Mutex::new(TestSensor {})));
        let data_coll_1 = DataCollector::new(
            "r1".to_string(),
            resource_1,
            CollectionMethod::Readings,
            10.0,
        );
        assert!(data_coll_1.is_ok());
        let data_coll_1 = data_coll_1.unwrap();

        let resource_2 = ResourceType::Sensor(Arc::new(Mutex::new(TestSensor {})));
        let data_coll_2 = DataCollector::new(
            "r2".to_string(),
            resource_2,
            CollectionMethod::Readings,
            50.0,
        );
        assert!(data_coll_2.is_ok());
        let data_coll_2 = data_coll_2.unwrap();

        let resource_3 = ResourceType::Sensor(Arc::new(Mutex::new(TestSensor {})));
        let data_coll_3 = DataCollector::new(
            "r2".to_string(),
            resource_3,
            CollectionMethod::Readings,
            10.0,
        );
        assert!(data_coll_3.is_ok());
        let data_coll_3 = data_coll_3.unwrap();

        let data_colls = vec![data_coll_1, data_coll_2, data_coll_3];
        let store = NoOpStore {};

        let data_manager = DataManager::new(
            data_colls,
            store,
            Duration::from_millis(30),
            "1".to_string(),
            Arc::new(futures_util::lock::Mutex::new(None))
        );
        assert!(data_manager.is_ok());
        let data_manager = data_manager.unwrap();
        let expected_collection_intervals: Vec<u64> = vec![20, 100];
        assert_eq!(
            data_manager.collection_intervals(),
            expected_collection_intervals
        );
    }

    #[test_log::test]
    fn test_collect_readings_for_interval() {
        let resource_1 = ResourceType::Sensor(Arc::new(Mutex::new(TestSensor {})));
        let data_coll_1 = DataCollector::new(
            "r1".to_string(),
            resource_1,
            CollectionMethod::Readings,
            10.0,
        );
        assert!(data_coll_1.is_ok());
        let data_coll_1 = data_coll_1.unwrap();
        let method_key_1 = data_coll_1.resource_method_key();

        let resource_2 = ResourceType::Sensor(Arc::new(Mutex::new(TestSensor {})));
        let data_coll_2 = DataCollector::new(
            "r2".to_string(),
            resource_2,
            CollectionMethod::Readings,
            50.0,
        );
        assert!(data_coll_2.is_ok());
        let data_coll_2 = data_coll_2.unwrap();
        let method_key_2 = data_coll_2.resource_method_key();

        let resource_3 = ResourceType::Sensor(Arc::new(Mutex::new(TestSensor {})));
        let data_coll_3 = DataCollector::new(
            "r2".to_string(),
            resource_3,
            CollectionMethod::Readings,
            10.0,
        );
        assert!(data_coll_3.is_ok());
        let data_coll_3 = data_coll_3.unwrap();

        let data_colls = vec![data_coll_1, data_coll_2, data_coll_3];
        let store = NoOpStore {};

        let data_manager = DataManager::new(
            data_colls,
            store,
            Duration::from_millis(30),
            "1".to_string(),
            Arc::new(futures_util::lock::Mutex::new(None))
        );
        assert!(data_manager.is_ok());
        let mut data_manager = data_manager.unwrap();

        let sensor_data = data_manager.collect_readings_for_interval(100);
        assert!(sensor_data.is_ok());
        let sensor_data = sensor_data.unwrap();
        assert_eq!(sensor_data.len(), 2);

        assert_eq!(sensor_data[0].0, method_key_1);
        assert!(sensor_data[0].1.data.is_some());
        let data = sensor_data[0].1.data.clone().unwrap();
        assert!(matches!(data, Data::Struct(_)));
        match data {
            Data::Struct(data) => {
                let data = data.fields;
                assert!(data.contains_key("readings"));
                let inner_data = data.get("readings").unwrap();
                assert!(inner_data.kind.is_some());
                let inner_data = inner_data.kind.clone().unwrap();
                assert!(matches!(inner_data, Kind::StructValue(_)));
                match inner_data {
                    Kind::StructValue(inner_data) => {
                        let val = inner_data.fields.get("thing");
                        assert!(val.is_some());
                        let val = &val.unwrap().kind;
                        assert!(val.is_some());
                        let val = val.clone().unwrap();
                        assert!(matches!(val, Kind::NumberValue(_)));
                        match val {
                            Kind::NumberValue(x) => assert_eq!(x, 42.42),
                            _ => unreachable!(),
                        };
                    }
                    _ => unreachable!(),
                };
            }
            _ => unreachable!(),
        };

        assert_eq!(sensor_data[1].0, method_key_2);
        assert!(sensor_data[1].1.data.is_some());
        let data = sensor_data[1].1.data.clone().unwrap();
        assert!(matches!(data, Data::Struct(_)));
        match data {
            Data::Struct(data) => {
                let data = data.fields;
                assert!(data.contains_key("readings"));
                let inner_data = data.get("readings").unwrap();
                assert!(inner_data.kind.is_some());
                let inner_data = inner_data.kind.clone().unwrap();
                assert!(matches!(inner_data, Kind::StructValue(_)));
                match inner_data {
                    Kind::StructValue(inner_data) => {
                        let val = inner_data.fields.get("thing");
                        assert!(val.is_some());
                        let val = &val.unwrap().kind;
                        assert!(val.is_some());
                        let val = val.clone().unwrap();
                        assert!(matches!(val, Kind::NumberValue(_)));
                        match val {
                            Kind::NumberValue(x) => assert_eq!(x, 42.42),
                            _ => unreachable!(),
                        };
                    }
                    _ => unreachable!(),
                };
            }
            _ => unreachable!(),
        };
    }

    #[test_log::test]
    fn test_collect_readings_for_interval_failure() {
        let resource_1 = ResourceType::Sensor(Arc::new(Mutex::new(TestSensorFailure {})));
        let data_coll_1 = DataCollector::new(
            "r1".to_string(),
            resource_1,
            CollectionMethod::Readings,
            10.0,
        );
        assert!(data_coll_1.is_ok());
        let data_coll_1 = data_coll_1.unwrap();
        let store = NoOpStore {};

        let resource_3 = ResourceType::Sensor(Arc::new(Mutex::new(TestSensor {})));
        let data_coll_3 = DataCollector::new(
            "r2".to_string(),
            resource_3,
            CollectionMethod::Readings,
            10.0,
        );
        assert!(data_coll_3.is_ok());
        let data_coll_3 = data_coll_3.unwrap();

        let data_manager = DataManager::new(
            vec![data_coll_1, data_coll_3],
            store,
            Duration::from_millis(30),
            "1".to_string(),
            Arc::new(futures_util::lock::Mutex::new(None))
        );
        assert!(data_manager.is_ok());
        let mut data_manager = data_manager.unwrap();

        let readings = data_manager.collect_readings_for_interval(100);
        assert!(readings.is_err());
    }

    #[derive(DoCommand)]
    struct TestSensor2 {}

    impl Sensor for TestSensor2 {}

    impl Readings for TestSensor2 {
        fn get_generic_readings(&mut self) -> Result<GenericReadingsResult, SensorError> {
            Ok(self
                .get_readings()?
                .into_iter()
                .map(|v| (v.0, SensorResult::<f64> { value: v.1 }.into()))
                .collect())
        }
    }

    impl SensorT<f64> for TestSensor2 {
        fn get_readings(&self) -> Result<TypedReadingsResult<f64>, SensorError> {
            let mut x = HashMap::new();
            x.insert("thing".to_string(), 24.24);
            Ok(x)
        }
    }

    impl Status for TestSensor2 {
        fn get_status(&self) -> Result<Option<Struct>, StatusError> {
            Err(StatusError::EncoderError(
                EncoderError::EncoderMethodUnimplemented,
            ))
        }
    }

    struct ReadSavingStore {
        store: LocalRb<SensorData, Vec<MaybeUninit<SensorData>>>,
        other_store: LocalRb<SensorData, Vec<MaybeUninit<SensorData>>>,
        read_messages: Vec<SensorData>,
    }

    impl ReadSavingStore {
        fn new() -> Self {
            Self {
                store: LocalRb::new(10),
                other_store: LocalRb::new(10),
                read_messages: Vec::new(),
            }
        }
        fn read_messages(&self) -> Vec<SensorData> {
            self.read_messages.clone()
        }
    }

    impl DataStore for ReadSavingStore {
        fn write_message(
            &mut self,
            collector_key: &ResourceMethodKey,
            message: SensorData,
            _write_mode: WriteMode,
        ) -> Result<(), DataStoreError> {
            let store = if &collector_key.r_name == "r1" {
                &mut self.store
            } else {
                &mut self.other_store
            };
            store
                .push(message)
                .map_err(|msg| DataStoreError::DataBufferFull(collector_key.clone(), msg))?;
            Ok(())
        }
        fn read_next_message(
            &mut self,
            collector_key: &ResourceMethodKey,
        ) -> Result<bytes::BytesMut, DataStoreError> {
            let store = if &collector_key.r_name == "r1" {
                &mut self.store
            } else {
                &mut self.other_store
            };
            match store.pop() {
                Some(msg) => {
                    self.read_messages.push(msg.clone());
                    let mut res = BytesMut::with_capacity(11);
                    res.put(&b"ignore this"[..]);
                    Ok(res)
                }
                None => Ok(BytesMut::with_capacity(0)),
            }
        }
        fn from_resource_method_keys(
            _collector_keys: Vec<ResourceMethodKey>,
        ) -> Result<Self, DataStoreError> {
            Ok(Self::new())
        }
    }

    fn get_values_from_manager(manager: &DataManager<ReadSavingStore>) -> Vec<f64> {
        let read_data = manager
            .store
            .read_messages()
            .into_iter()
            .map(|msg| msg.data.unwrap());
        let read_data: Vec<f64> = read_data
            .into_iter()
            .map(|d| match d {
                Data::Binary(_) => 0.0,
                Data::Struct(s) => match s.fields.get("readings").unwrap().kind.as_ref().unwrap() {
                    Kind::StructValue(s) => {
                        match s.fields.get("thing").unwrap().kind.as_ref().unwrap() {
                            Kind::NumberValue(n) => *n,
                            _ => 0.0,
                        }
                    }
                    _ => 0.0,
                },
            })
            .collect();
        read_data
    }

    #[test_log::test]
    fn test_run_inner() {
        let resource_1 = ResourceType::Sensor(Arc::new(Mutex::new(TestSensor {})));
        let resource_2 = ResourceType::Sensor(Arc::new(Mutex::new(TestSensor2 {})));

        let data_coll_1 = DataCollector::new(
            "r1".to_string(),
            resource_1,
            CollectionMethod::Readings,
            50.0,
        );
        assert!(data_coll_1.is_ok());
        let data_coll_1 = data_coll_1.unwrap();

        let data_coll_2 = DataCollector::new(
            "r2".to_string(),
            resource_2,
            CollectionMethod::Readings,
            20.0,
        );
        assert!(data_coll_2.is_ok());
        let data_coll_2 = data_coll_2.unwrap();
        
        async_io::block_on(async move {
            let manager = DataManager::new(
                vec![data_coll_1, data_coll_2],
                ReadSavingStore::new(),
                Duration::from_millis(65),
                "boop".to_string(),
                Arc::new(futures_util::lock::Mutex::new(None))
            );
            assert!(manager.is_ok());
            let mut manager = manager.unwrap();
    
            let expected_data: Vec<f64> = vec![
                42.42, 42.42, 42.42, 24.24, 24.24, 42.42, 42.42, 42.42, 24.24,
            ];
            for i in 0..7 {
                assert!(manager.run_inner(i, true).await.is_ok());
            }
            let read_data = get_values_from_manager(&manager);
            assert_eq!(read_data, expected_data);
        });
    }
}
