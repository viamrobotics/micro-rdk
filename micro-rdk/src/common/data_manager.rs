use std::pin::Pin;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::common::data_collector::{DataCollectionError, DataCollector};
use crate::common::data_store::DataStore;
use crate::google::protobuf::value::Kind;
use crate::proto::app::data_sync::v1::{
    DataCaptureUploadRequest, DataType, SensorData, UploadMetadata,
};
use crate::proto::app::v1::ConfigResponse;

use super::app_client::{AppClient, AppClientConfig, AppClientError, PeriodicAppClientTask};
use super::data_collector::ResourceMethodKey;
use super::data_store::{DataStoreError, DataStoreReader, WriteMode};
use super::robot::{LocalRobot, RobotError};
use async_io::Timer;
use bytes::BytesMut;
use futures_lite::prelude::Future;
use futures_util::lock::Mutex as AsyncMutex;
use prost::Message;
use thiserror::Error;

// Maximum size in bytes of readings that should be sent in a single request
// as recommended by Viam's data management team is 64K. To accommodate for
// the smaller amount of available RAM, we've halved it
static MAX_SENSOR_CONTENTS_SIZE: usize = 32000;

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

pub struct DataManager<StoreType> {
    collectors: Vec<DataCollector>,
    store: Rc<AsyncMutex<StoreType>>,
    sync_interval: Duration,
    min_interval: Duration,
    part_id: String,
}

impl<StoreType> DataManager<StoreType>
where
    StoreType: DataStore,
{
    pub fn new(
        collectors: Vec<DataCollector>,
        store: StoreType,
        sync_interval: Duration,
        part_id: String,
    ) -> Result<Self, DataManagerError> {
        let intervals = collectors.iter().map(|x| x.time_interval());
        let min_interval = intervals.min().ok_or(DataManagerError::NoCollectors)?;
        Ok(Self {
            collectors,
            store: Rc::new(AsyncMutex::new(store)),
            sync_interval,
            min_interval,
            part_id,
        })
    }

    pub fn from_robot_and_config(
        cfg: &ConfigResponse,
        app_config: &AppClientConfig,
        robot: Arc<Mutex<LocalRobot>>,
    ) -> Result<Option<Self>, DataManagerError> {
        let part_id = app_config.get_robot_id();
        let sync_interval = get_data_sync_interval(cfg)?;
        if let Some(sync_interval) = sync_interval {
            let collectors = robot.lock().unwrap().data_collectors()?;
            let collector_keys: Vec<ResourceMethodKey> =
                collectors.iter().map(|c| c.resource_method_key()).collect();
            let store = StoreType::from_resource_method_keys(collector_keys)?;
            let data_manager_svc = DataManager::new(collectors, store, sync_interval, part_id)?;
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

    pub async fn data_collection_task(&mut self) -> Result<(), DataManagerError> {
        let mut loop_counter: u64 = 0;
        loop {
            self.collect_data_inner(loop_counter).await?;
            loop_counter += 1;
            Timer::after(self.min_interval).await;
        }
    }

    pub async fn collect_data_inner(&mut self, loop_counter: u64) -> Result<(), DataManagerError> {
        let min_interval_ms = self.min_interval_ms();
        for interval in self.collection_intervals() {
            if loop_counter % (interval / min_interval_ms) == 0 {
                self.collect_and_store_readings(interval).await?;
            }
        }
        Ok(())
    }

    async fn collect_and_store_readings(
        &mut self,
        time_interval_ms: u64,
    ) -> Result<(), DataManagerError> {
        let readings = self.collect_readings_for_interval(time_interval_ms)?;
        let mut store_guard = self.store.lock().await;
        for (collector_key, reading) in readings {
            store_guard.write_message(&collector_key, reading, WriteMode::OverwriteOldest)?;
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

    pub fn get_sync_task(&self) -> DataSyncTask<StoreType> {
        let resource_method_keys: Vec<ResourceMethodKey> = self
            .collectors
            .iter()
            .map(|coll| coll.resource_method_key())
            .collect();
        DataSyncTask {
            store: self.store.clone(),
            resource_method_keys,
            sync_interval: self.sync_interval,
            part_id: self.part_id(),
        }
    }
}

#[derive(Debug, Error)]
pub enum DataSyncError {
    #[error(transparent)]
    MessageDecodingError(#[from] prost::DecodeError),
    #[error(transparent)]
    UploadError(#[from] AppClientError),
}

pub struct DataSyncTask<StoreType> {
    store: Rc<AsyncMutex<StoreType>>,
    resource_method_keys: Vec<ResourceMethodKey>,
    sync_interval: Duration,
    part_id: String,
}

async fn upload_data<'b>(
    app_client: &'b AppClient,
    part_id: String,
    collector_key: &ResourceMethodKey,
    raw_data: &mut Vec<BytesMut>,
) -> Result<(), DataSyncError> {
    let data: Result<Vec<SensorData>, prost::DecodeError> =
        raw_data.drain(..).map(SensorData::decode).collect();

    let upload_request = DataCaptureUploadRequest {
        metadata: Some(UploadMetadata {
            part_id: part_id,
            component_type: collector_key.component_type.clone(),
            r#type: DataType::TabularSensor.into(),
            component_name: collector_key.r_name.clone(),
            method_name: collector_key.method.to_string(),
            ..Default::default()
        }),
        sensor_contents: data?,
    };
    app_client.upload_data(upload_request).await?;
    log::info!(
        "successfully uploaded data, collector key: {:?}",
        collector_key
    );
    Ok(())
}

impl<StoreType> DataSyncTask<StoreType>
where
    StoreType: DataStore,
{
    async fn run<'b>(&mut self, app_client: &'b AppClient) -> Result<(), AppClientError> {
        for collector_key in self.resource_method_keys.iter() {
            let store_lock = self.store.lock().await;
            let mut current_chunk: Vec<BytesMut> = vec![];
            let mut chunk_size_counter: usize = 0;
            let mut reader = match store_lock.get_reader(collector_key) {
                Ok(reader) => reader,
                Err(err) => {
                    log::error!("error acquiring store reader: {:?}", err);
                    continue;
                }
            };
            loop {
                let next_message = match reader.read_next_message() {
                    Ok(msg) => msg,
                    Err(err) => {
                        log::error!("error reading from store: {:?}", err);
                        break;
                    }
                };
                if next_message.is_empty() {
                    break;
                }
                // If the amount of data becomes too big for a single upload, we want to start a new
                // collection of upload data
                if (chunk_size_counter + next_message.len()) > MAX_SENSOR_CONTENTS_SIZE {
                    match upload_data(
                        app_client,
                        self.part_id.clone(),
                        collector_key,
                        &mut current_chunk,
                    )
                    .await
                    {
                        Ok(_) => {
                            chunk_size_counter = 0;
                            reader.flush();
                        }
                        Err(DataSyncError::UploadError(err)) => {
                            // if there is an error actually uploading the data, we want to keep it in the store
                            return Err(err);
                        }
                        Err(err) => {
                            log::error!("error processing data chunk {:?}", err);
                            chunk_size_counter = 0;
                            reader.flush();
                        }
                    };
                } else {
                    chunk_size_counter += next_message.len();
                    current_chunk.push(next_message);
                }
            }
            if !current_chunk.is_empty() {
                match upload_data(
                    app_client,
                    self.part_id.clone(),
                    collector_key,
                    &mut current_chunk,
                )
                .await
                {
                    Ok(_) => {
                        reader.flush();
                    }
                    Err(DataSyncError::UploadError(err)) => {
                        return Err(err);
                    }
                    Err(err) => {
                        log::error!("error processing data chunk {:?}", err);
                    }
                };
            }
        }
        Ok(())
    }
}

impl<StoreType> PeriodicAppClientTask for DataSyncTask<StoreType>
where
    StoreType: DataStore,
{
    fn name(&self) -> &str {
        "DataSync"
    }
    fn get_default_period(&self) -> Duration {
        self.sync_interval
    }
    fn invoke<'b, 'a: 'b>(
        &'a mut self,
        app_client: &'b AppClient,
    ) -> Pin<Box<dyn Future<Output = Result<Option<Duration>, AppClientError>> + 'b>> {
        Box::pin(async move { self.run(app_client).await.map(|_| None) })
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::mem::MaybeUninit;
    use std::rc::Rc;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use bytes::BytesMut;
    use prost::Message;
    use ringbuf::{LocalRb, Rb};

    use super::DataManager;
    use crate::common::data_store::{DataStoreReader, WriteMode};
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

    struct NoOpReader {}

    impl DataStoreReader for NoOpReader {
        fn read_next_message(&mut self) -> Result<BytesMut, DataStoreError> {
            Err(DataStoreError::Unimplemented)
        }
        fn flush(&mut self) {}
    }

    struct NoOpStore {}

    impl DataStore for NoOpStore {
        type Reader = NoOpReader;
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
        fn get_reader(
            &self,
            _collector_key: &ResourceMethodKey,
        ) -> Result<NoOpReader, DataStoreError> {
            Ok(NoOpReader {})
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

    lazy_static::lazy_static! {
        static ref READ_MESSAGES: Mutex<Vec<SensorData>> = Mutex::new(vec![]);
    }

    struct ReadSavingStoreReader {
        store: Rc<RefCell<LocalRb<SensorData, Vec<MaybeUninit<SensorData>>>>>,
    }

    impl DataStoreReader for ReadSavingStoreReader {
        fn read_next_message(&mut self) -> Result<BytesMut, DataStoreError> {
            match RefCell::borrow_mut(&self.store).pop() {
                Some(msg) => {
                    READ_MESSAGES.lock().unwrap().push(msg.clone());
                    let mut res = BytesMut::with_capacity(msg.encoded_len());
                    msg.encode(&mut res)?;
                    Ok(res)
                }
                None => Ok(BytesMut::with_capacity(0)),
            }
        }
        fn flush(&mut self) {}
    }

    struct ReadSavingStore {
        store: Rc<RefCell<LocalRb<SensorData, Vec<MaybeUninit<SensorData>>>>>,
        other_store: Rc<RefCell<LocalRb<SensorData, Vec<MaybeUninit<SensorData>>>>>,
    }

    impl ReadSavingStore {
        fn new() -> Self {
            Self {
                store: Rc::new(RefCell::new(LocalRb::new(10))),
                other_store: Rc::new(RefCell::new(LocalRb::new(10))),
            }
        }
    }

    impl DataStore for ReadSavingStore {
        type Reader = ReadSavingStoreReader;
        fn write_message(
            &mut self,
            collector_key: &ResourceMethodKey,
            message: SensorData,
            _write_mode: WriteMode,
        ) -> Result<(), DataStoreError> {
            RefCell::borrow_mut(if &collector_key.r_name == "r1" {
                &self.store
            } else {
                &self.other_store
            })
            .push(message)
            .map_err(|_| DataStoreError::DataBufferFull(collector_key.clone()))?;
            Ok(())
        }
        fn from_resource_method_keys(
            _collector_keys: Vec<ResourceMethodKey>,
        ) -> Result<Self, DataStoreError> {
            Ok(Self::new())
        }
        fn get_reader(
            &self,
            collector_key: &ResourceMethodKey,
        ) -> Result<Self::Reader, DataStoreError> {
            let store_clone = if &collector_key.r_name == "r1" {
                self.store.clone()
            } else {
                self.other_store.clone()
            };
            Ok(ReadSavingStoreReader { store: store_clone })
        }
    }

    fn get_read_values() -> Vec<f64> {
        let message_vec = READ_MESSAGES.lock().unwrap();
        let read_data = message_vec.iter().map(|msg| msg.data.clone().unwrap());
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
    fn test_reader() {
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
        let coll_key_1 = data_coll_1.resource_method_key();

        let data_coll_2 = DataCollector::new(
            "r2".to_string(),
            resource_2,
            CollectionMethod::Readings,
            20.0,
        );
        assert!(data_coll_2.is_ok());
        let data_coll_2 = data_coll_2.unwrap();
        let coll_key_2 = data_coll_2.resource_method_key();

        let manager = DataManager::new(
            vec![data_coll_1, data_coll_2],
            ReadSavingStore::new(),
            Duration::from_millis(65),
            "boop".to_string(),
        );
        assert!(manager.is_ok());
        let mut manager = manager.unwrap();

        let sync_task = manager.get_sync_task();

        let min_interval_ms = manager.min_interval_ms();

        async_io::block_on(async move {
            for i in 0..7 {
                if (i == 3) || (i == 6) {
                    let res = sync_task.read_messages_for_collector(&coll_key_1).await;
                    assert!(res.is_ok());
                    let res = sync_task.read_messages_for_collector(&coll_key_2).await;
                    assert!(res.is_ok());
                }
                assert!(manager.collect_data_inner(i).await.is_ok())
            }

            let expected_data: Vec<f64> = vec![
                42.42, 42.42, 42.42, 24.24, 24.24, 42.42, 42.42, 42.42, 24.24,
            ];
            let read_data = get_read_values();
            assert_eq!(read_data, expected_data);
        });
    }
}
