use std::pin::Pin;
use std::rc::Rc;
use std::time::{Duration, Instant};

use crate::common::data_collector::{DataCollectionError, DataCollector};
use crate::common::data_store::DataStore;
use crate::google::protobuf::value::Kind;
use crate::google::protobuf::{Struct, Timestamp};
use crate::proto::app::data_sync::v1::{
    DataCaptureUploadRequest, DataType, SensorData, UploadMetadata,
};
use crate::proto::app::v1::{RobotConfig, ServiceConfig};

use super::app_client::{AppClient, AppClientError, PeriodicAppClientTask, VIAM_FOUNDING_YEAR};
use super::data_collector::ResourceMethodKey;
use super::data_store::{DataStoreError, DataStoreReader, WriteMode};
use super::robot::{LocalRobot, RobotError};
use super::system::{send_system_event, SystemEvent};
use async_io::Timer;
use bytes::BytesMut;
use chrono::offset::Local;
use chrono::Datelike;
use futures_lite::prelude::Future;
use futures_util::lock::Mutex as AsyncMutex;
use prost::Message;
use thiserror::Error;

// Maximum size in bytes of readings that should be sent in a single request
// as recommended by Viam's data management team is 64K. To accommodate for
// the smaller amount of available RAM, we've halved it
static MAX_SENSOR_CONTENTS_SIZE: usize = 32000;

type CollectedReadings = Vec<(
    ResourceMethodKey,
    Result<Vec<SensorData>, DataCollectionError>,
)>;

/// Allow for a C project using micro-RDK as a library to implement a callback to be run
/// whenever data has successfully been uploaded. The callback should be identical in signature
/// to the weak symbol declared here
///
/// Ex. for ESP-IDF project
/// void micro_rdk_data_manager_post_upload_hook(void) {
///     ESP_LOGI("proj_main", "hit data upload callback");
/// }
///
/// # Safety
/// The implemented callback should be simple. Long-running operations should be avoided, as should
/// any interaction with resources managed by micro-RDK or the taking of any resources protected by lock.
#[cfg(feature = "data-upload-hook-unstable")]
#[linkage = "weak"]
#[no_mangle]
pub unsafe extern "C" fn micro_rdk_data_manager_post_upload_hook() {}

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
    #[error("sync_interval required for collection and immediate data sync")]
    MissingSyncInterval,
    #[error("all collection intervals required to be equal to sync interval ({0} secs) for this mode of data collection")]
    CollectionSyncIntervalUnequal(u64),
}

fn get_data_service_config(
    robot_config: &RobotConfig,
) -> Result<Option<ServiceConfig>, DataManagerError> {
    let num_configs_detected = robot_config
        .services
        .iter()
        .filter(|svc_cfg| svc_cfg.r#type == *"data_manager")
        .count();
    if num_configs_detected > 1 {
        return Err(DataManagerError::MultipleConfigError);
    }
    Ok(robot_config
        .services
        .iter()
        .find(|svc_cfg| svc_cfg.r#type == *"data_manager")
        .cloned())
}

fn get_data_sync_interval(attrs: &Struct) -> Result<Option<Duration>, DataManagerError> {
    Ok(
        // If cloud sync is disabled, we'll communicate this by having the sync interval be None
        if attrs
            .fields
            .get("sync_disabled")
            .map(|v| match v.kind {
                Some(Kind::BoolValue(b)) => b,
                _ => false,
            })
            .unwrap_or(false)
        {
            None
        } else {
            let sync_interval_mins = attrs
                .fields
                .get("sync_interval_mins")
                .ok_or(DataManagerError::ConfigError)?;
            if let Some(Kind::NumberValue(sync_interval_mins)) = sync_interval_mins.kind {
                Some(Duration::from_secs((sync_interval_mins * 60.0) as u64))
            } else {
                return Err(DataManagerError::ConfigError);
            }
        },
    )
}

fn get_collectors_and_settings_from_robot_and_config(
    robot: &LocalRobot,
    cfg: &ServiceConfig,
) -> Result<(Vec<DataCollector>, Option<Duration>, String), DataManagerError> {
    let attrs = cfg
        .attributes
        .as_ref()
        .ok_or(DataManagerError::ConfigError)?;
    let sync_interval = get_data_sync_interval(attrs)?;
    let collectors = if attrs
        .fields
        .get("capture_disabled")
        .map(|v| match v.kind {
            Some(Kind::BoolValue(b)) => b,
            _ => false,
        })
        .unwrap_or(false)
    {
        vec![]
    } else {
        robot.data_collectors()?
    };

    Ok((collectors, sync_interval, robot.part_id.clone()))
}

pub struct DataManager<StoreType> {
    collectors: Vec<DataCollector>,
    store: Rc<AsyncMutex<StoreType>>,
    sync_interval: Option<Duration>,
    min_interval: Duration,
    robot_part_id: String,
}

impl<StoreType> DataManager<StoreType>
where
    StoreType: DataStore,
{
    pub fn new(
        collectors: Vec<DataCollector>,
        store: StoreType,
        sync_interval: Option<Duration>,
        robot_part_id: String,
    ) -> Result<Self, DataManagerError> {
        let intervals = collectors.iter().map(|x| x.time_interval());
        let min_interval = intervals.min().ok_or(DataManagerError::NoCollectors)?;
        Ok(Self {
            collectors,
            store: Rc::new(AsyncMutex::new(store)),
            sync_interval,
            min_interval,
            robot_part_id,
        })
    }

    pub fn from_robot_and_config(
        robot: &LocalRobot,
        cfg: &RobotConfig,
    ) -> Result<Option<Self>, DataManagerError> {
        if let Some(cfg) = get_data_service_config(cfg)? {
            let (collectors, sync_interval, part_id) =
                get_collectors_and_settings_from_robot_and_config(robot, &cfg)?;

            // if there are no collectors and cloud sync is off, simply don't create a DataManager
            if collectors.is_empty() && sync_interval.is_none() {
                Ok(None)
            } else {
                let collector_settings: Vec<(ResourceMethodKey, usize)> = collectors
                    .iter()
                    .map(|c| (c.resource_method_key(), c.capacity()))
                    .collect();
                let store = StoreType::from_resource_method_settings(collector_settings)?;
                let data_manager_svc = DataManager::new(collectors, store, sync_interval, part_id)?;
                Ok(Some(data_manager_svc))
            }
        } else {
            Ok(None)
        }
    }

    pub fn sync_interval_ms(&self) -> u64 {
        self.sync_interval.unwrap_or_default().as_millis() as u64
    }

    pub fn min_interval_ms(&self) -> u64 {
        self.min_interval.as_millis() as u64
    }

    pub fn part_id(&self) -> String {
        self.robot_part_id.clone()
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

    pub async fn data_collection_task(&mut self, robot_start_time: Instant) -> ! {
        let mut loop_counter: u64 = 0;
        loop {
            if let Err(e) = self
                .collect_data_inner(loop_counter, robot_start_time)
                .await
            {
                log::error!(
                    "data manager error {:?}, will attempt to continue collecting",
                    e
                );
            }
            loop_counter += 1;
            Timer::after(self.min_interval).await;
        }
    }

    pub async fn collect_data_inner(
        &mut self,
        loop_counter: u64,
        robot_start_time: Instant,
    ) -> Result<(), DataManagerError> {
        let min_interval_ms = self.min_interval_ms();
        for interval in self.collection_intervals() {
            if loop_counter % (interval / min_interval_ms) == 0 {
                self.collect_and_store_readings(interval, robot_start_time)
                    .await?;
            }
        }
        Ok(())
    }

    async fn collect_and_store_readings(
        &mut self,
        time_interval_ms: u64,
        robot_start_time: Instant,
    ) -> Result<(), DataManagerError> {
        let readings = self.collect_readings_for_interval(time_interval_ms, robot_start_time)?;
        let mut store_guard = self.store.lock().await;
        for (collector_key, reading) in readings {
            match reading {
                Err(e) => log::error!(
                    "collector {} failed to collect data reason {:?}",
                    &collector_key,
                    e
                ),
                Ok(data_vec) => {
                    for data in data_vec {
                        if let Err(e) = store_guard.write_message(
                            &collector_key,
                            data,
                            WriteMode::OverwriteOldest,
                        ) {
                            log::error!(
                                "couldn't store some data for collector {:?} error : {:?}",
                                collector_key,
                                e
                            );
                        }
                    }
                }
            }
        }
        Ok(())
    }

    // Here, time_interval_ms is required to be a multiple of the minimum time_interval among the collectors.
    // This function then collects readings from collectors whose time_interval is greater than time_interval_ms but
    // less than the next largest multiple of self.min_interval_ms
    fn collect_readings_for_interval(
        &mut self,
        time_interval_ms: u64,
        robot_start_time: Instant,
    ) -> Result<CollectedReadings, DataManagerError> {
        let min_interval_ms = self.min_interval_ms();
        if time_interval_ms % min_interval_ms != 0 {
            return Err(DataManagerError::ImproperTimeInterval(
                time_interval_ms,
                min_interval_ms,
            ));
        }

        Ok(self
            .collectors
            .iter_mut()
            .filter_map(|coll| {
                if (coll.time_interval().as_millis() as u64 / min_interval_ms)
                    == (time_interval_ms / min_interval_ms)
                {
                    Some((
                        coll.resource_method_key(),
                        coll.call_method(robot_start_time),
                    ))
                } else {
                    None
                }
            })
            .collect())
    }

    pub fn get_sync_task(&self, robot_start_time: Instant) -> Option<DataSyncTask<StoreType>> {
        if let Some(sync_interval) = self.sync_interval {
            let resource_method_keys: Vec<ResourceMethodKey> = self
                .collectors
                .iter()
                .map(|coll| coll.resource_method_key())
                .collect();
            Some(DataSyncTask {
                store: self.store.clone(),
                resource_method_keys,
                sync_interval,
                part_id: self.part_id(),
                robot_start_time,
            })
        } else {
            None
        }
    }
}

#[derive(Debug, Error)]
pub enum DataSyncError {
    #[error(transparent)]
    DataStoreError(#[from] DataStoreError),
    #[error(transparent)]
    MessageDecodingError(#[from] prost::DecodeError),
    #[error("time correction resulted in out of bounds duration")]
    TimeOutOfBoundsError,
    #[error("current time unset")]
    NoCurrentTime,
}

fn get_time_to_subtract(
    robot_start_time: Instant,
    stored_time: Timestamp,
) -> Result<Option<chrono::Duration>, DataSyncError> {
    let stored_time_dur = Duration::new(stored_time.seconds as u64, stored_time.nanos as u32);
    // this can sometimes happen when the source of the timestamp is not from the current run of firmware code,
    // such as data collected using code running on ULP. In this case we don't want to do any time correction
    // and should defer to the existing timestamp
    if robot_start_time
        .elapsed()
        .checked_sub(stored_time_dur)
        .is_none()
    {
        return Ok(None);
    }
    let time_to_subtract = robot_start_time.elapsed() - stored_time_dur;
    let time_to_subtract = chrono::Duration::new(
        time_to_subtract.as_secs() as i64,
        time_to_subtract.subsec_nanos(),
    )
    .ok_or(DataSyncError::TimeOutOfBoundsError)?;
    Ok(Some(time_to_subtract))
}

fn time_correct_reading(
    robot_start_time: Instant,
    msg: &mut SensorData,
) -> Result<(), DataSyncError> {
    // the timestamps of the stored data are measured as offsets from a starting
    // instant (robot_start_time, acquired from DataSyncTask), so we adjust the
    // timestamps on the parsed message based on the current time (if it is now available)
    if let Some(metadata) = msg.metadata.as_mut() {
        let current_dt = Local::now().fixed_offset();
        // Viam was founded in 2020, so if the current time is set to any time before that
        // we know that settimeofday was never called, or called with an improper datetime
        if current_dt.year() < VIAM_FOUNDING_YEAR {
            return Err(DataSyncError::NoCurrentTime);
        }

        if let Some(time_received) = metadata.time_received.clone() {
            if let Some(time_to_subtract) = get_time_to_subtract(robot_start_time, time_received)? {
                let time_received = current_dt - time_to_subtract;
                metadata.time_received = Some(Timestamp {
                    seconds: time_received.timestamp(),
                    nanos: time_received.timestamp_subsec_nanos() as i32,
                });
            };
        }
        if let Some(time_requested) = metadata.time_requested.clone() {
            if let Some(time_to_subtract) = get_time_to_subtract(robot_start_time, time_requested)?
            {
                let time_requested = current_dt - time_to_subtract;
                metadata.time_requested = Some(Timestamp {
                    seconds: time_requested.timestamp(),
                    nanos: time_requested.timestamp_subsec_nanos() as i32,
                });
            };
        }
    }
    Ok(())
}

pub struct DataSyncTask<StoreType> {
    store: Rc<AsyncMutex<StoreType>>,
    resource_method_keys: Vec<ResourceMethodKey>,
    sync_interval: Duration,
    part_id: String,
    // used for time correcting stored data before upload, see DataSyncTask::run
    // and create_time_corrected_reading below
    robot_start_time: Instant,
}

impl<StoreType> DataSyncTask<StoreType>
where
    StoreType: DataStore,
{
    #[cfg(test)]
    async fn get_store_lock(&mut self) -> futures_util::lock::MutexGuard<StoreType> {
        self.store.lock().await
    }

    fn get_time_corrected_reading(&self, raw_msg: BytesMut) -> Result<SensorData, DataSyncError> {
        let mut msg = SensorData::decode(raw_msg)?;
        time_correct_reading(self.robot_start_time, &mut msg)?;
        Ok(msg)
    }

    async fn run(&self, app_client: &AppClient) -> Result<(), AppClientError> {
        for collector_key in self.resource_method_keys.iter() {
            // Since a write may occur in between uploading consecutive chunks of data, we want to make
            // sure only to process the messages initially present in this region of the store.
            let total_messages = {
                let store_lock = self.store.lock().await;
                match store_lock.get_reader(collector_key) {
                    Ok(reader) => match reader.messages_remaining() {
                        Ok(num_msgs) => num_msgs,
                        Err(err) => {
                            log::error!("could not get number of messages remaining in store for collector key ({:?}): {:?}", collector_key, err);
                            0
                        }
                    },
                    Err(err) => {
                        log::error!(
                            "error acquiring reader for collector key ({:?}): {:?}",
                            collector_key,
                            err
                        );
                        0
                    }
                }
            };
            if total_messages == 0 {
                continue;
            }
            let max_messages_per_chunk = std::cmp::max(10, total_messages.div_ceil(10));
            let mut messages_processed = 0;

            let mut current_chunk: Vec<BytesMut> = vec![];
            // we process the data for this region of the store in chunks, each iteration of this loop
            // should represent the processing and uploading of a single chunk of data
            loop {
                let store_lock = self.store.lock().await;
                let mut reader = match store_lock.get_reader(collector_key) {
                    Ok(reader) => reader,
                    Err(err) => {
                        log::error!(
                            "error acquiring reader for collector key ({:?}): {:?}",
                            collector_key,
                            err
                        );
                        break;
                    }
                };
                let next_message = match reader.read_next_message() {
                    Ok(msg) => {
                        // this can occur when the last message in the store was the last message
                        // in the previously uploaded chunk
                        if msg.is_empty() {
                            break;
                        }
                        messages_processed += 1;
                        msg
                    }
                    Err(err) => {
                        log::error!(
                            "error reading message from store for collector key ({:?}): {:?}",
                            collector_key,
                            err
                        );

                        // we don't want to panic, and creating an AppClientError variant for this case
                        // feels too specific, so we'll move on to the next collector
                        break;
                    }
                };

                if next_message.len() > MAX_SENSOR_CONTENTS_SIZE {
                    log::error!(
                        "message encountered that was too large (>32K bytes) for collector {:?}",
                        collector_key
                    );
                } else {
                    current_chunk.push(next_message);
                }

                // We want to fill current_chunk until its size reaches just under
                // MAX_SENSOR_CONTENTS_SIZE and then upload the data. Since we will have
                // needed to pull the first message of the next chunk to realize that
                // we have reached capacity, we return that message as well to be placed
                // at the beginning of the now empty current_chunk once we have successfully
                // uploaded it
                let (upload_data, next_chunk_first_message) = loop {
                    let next_message = match reader.read_next_message() {
                        Ok(msg) => {
                            messages_processed += 1;
                            msg
                        }
                        Err(err) => {
                            log::error!(
                                "error reading message from store for collector key ({:?}): {:?}",
                                collector_key,
                                err
                            );

                            // we don't want to panic, and creating an AppClientError variant for this case
                            // feels too specific, so we'll move on to the next collector without flushing
                            // this region of the store
                            break (vec![], None);
                        }
                    };

                    // skip this message if it's too big
                    if next_message.len() > MAX_SENSOR_CONTENTS_SIZE {
                        log::error!(
                            "message encountered that was too large (>32K bytes) for collector {:?}",
                            collector_key
                        );
                        continue;
                    }
                    let current_chunk_size: usize = current_chunk.iter().map(|c| c.len()).sum();
                    if (messages_processed >= total_messages)
                        || ((next_message.len() + current_chunk_size) > MAX_SENSOR_CONTENTS_SIZE)
                        || ((current_chunk.len() + 1) > max_messages_per_chunk)
                        || (next_message.is_empty())
                    {
                        let data: Result<Vec<SensorData>, DataSyncError> = current_chunk
                            .drain(..)
                            .map(|msg| self.get_time_corrected_reading(msg))
                            .collect();
                        let data = match data {
                            Ok(data) => data,
                            Err(DataSyncError::NoCurrentTime) => {
                                log::error!("Could not calculate data timestamps, returning without flushing store");
                                return Ok(());
                            }
                            Err(err) => {
                                log::error!(
                                    "error decoding readings for collector key ({:?}): {:?}",
                                    collector_key,
                                    err
                                );
                                vec![]
                            }
                        };
                        if next_message.is_empty() {
                            break (data, None);
                        }
                        break (data, Some(next_message));
                    } else {
                        current_chunk.push(next_message);
                    }
                };

                // We don't want to hold on to the store lock over a potentially long-running upload attempt.
                // However, when a request is sent using the hyper library, the memory representing the request data
                // is only cleaned up when the future to send the request somehow resolves. So if we put our own
                // timeout on this upload request, this leaked data will accumulate on every subsequent failed
                // upload attempt. To accomodate this, we flush the store before attempting to upload the data
                // and accept that the current chunk of data (in addition to the very first message of the next chunk)
                // will be lost on failure. Because an inability to connect to app will result in no longer having an
                // AppClient available, this task will not attempt to run again and additional data loss will be prevented
                reader.flush();
                std::mem::drop(store_lock);

                if !upload_data.is_empty() {
                    let data_len = upload_data.len();
                    let upload_request = DataCaptureUploadRequest {
                        metadata: Some(UploadMetadata {
                            part_id: self.part_id.clone(),
                            component_type: collector_key.component_type.clone(),
                            r#type: DataType::TabularSensor.into(),
                            component_name: collector_key.r_name.clone(),
                            method_name: collector_key.method.to_string(),
                            ..Default::default()
                        }),
                        sensor_contents: upload_data,
                    };
                    match app_client.upload_data(upload_request).await {
                        Ok(_) => {
                            if let Some(next_message) = next_chunk_first_message {
                                current_chunk = vec![next_message];
                            }
                            #[cfg(feature = "data-upload-hook-unstable")]
                            unsafe {
                                micro_rdk_data_manager_post_upload_hook();
                            }
                        }
                        Err(err) => {
                            log::error!(
                                "error uploading data, data lost ({:?} messages)",
                                data_len + 1
                            );
                            return Err(err);
                        }
                    };
                }
                if messages_processed >= total_messages {
                    break;
                }
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
        &'a self,
        app_client: &'b AppClient,
    ) -> Pin<Box<dyn Future<Output = Result<Option<Duration>, AppClientError>> + 'b>> {
        Box::pin(async move { self.run(app_client).await.map(|_| None) })
    }
}

// TODO: Unify DataCollectAndSyncTask and DataSyncTask into a single task to minimize divergence
// due to different firmware modes
pub struct DataCollectAndSyncTask {
    collectors: Vec<DataCollector>,
    sync_interval: Duration,
    robot_start_time: Instant,
    part_id: String,
}

impl DataCollectAndSyncTask {
    async fn upload_data(
        &self,
        app_client: &AppClient,
        collector_key: &ResourceMethodKey,
        data: Vec<SensorData>,
    ) -> Result<(), AppClientError> {
        let upload_request = DataCaptureUploadRequest {
            metadata: Some(UploadMetadata {
                part_id: self.part_id.clone(),
                component_type: collector_key.component_type.clone(),
                r#type: DataType::TabularSensor.into(),
                component_name: collector_key.r_name.clone(),
                method_name: collector_key.method.to_string(),
                ..Default::default()
            }),
            sensor_contents: data,
        };
        match app_client.upload_data(upload_request).await {
            Ok(_) => {
                #[cfg(feature = "data-upload-hook-unstable")]
                unsafe {
                    micro_rdk_data_manager_post_upload_hook();
                }
                Ok(())
            }
            Err(err) => Err(err),
        }
    }

    async fn run(&self, app_client: &AppClient) -> Result<(), AppClientError> {
        let collectors_len = self.collectors.len();
        for (idx, collector) in self.collectors.iter().enumerate() {
            let collector_key = collector.resource_method_key();
            let readings = collector
                .call_method(self.robot_start_time)
                .inspect_err(|err| {
                    log::error!(
                        "failed to collect readings for {:?}: {:?}",
                        collector_key,
                        err
                    )
                });
            if let Ok(readings) = readings {
                let robot_start_time = self.robot_start_time;
                let mut readings: Vec<SensorData> = readings
                    .into_iter()
                    .filter_map(|mut readings| {
                        if let Err(err) = time_correct_reading(robot_start_time, &mut readings) {
                            log::error!(
                                "unable to time correct reading for collector {:?}, {:?}",
                                collector_key,
                                err
                            );
                            None
                        } else {
                            Some(readings)
                        }
                    })
                    .collect();
                readings.reverse();
                if !readings.is_empty() {
                    let mut readings_to_upload: Vec<SensorData> = vec![];
                    let mut current_readings_size = 0;
                    while !readings.is_empty() {
                        if let Some(next_reading) = readings.pop() {
                            if current_readings_size + next_reading.encoded_len()
                                > MAX_SENSOR_CONTENTS_SIZE
                            {
                                let current_upload = readings_to_upload.split_off(0);
                                // if we can't upload, don't try again until after the next sleep
                                if let Err(err) = self
                                    .upload_data(app_client, &collector_key, current_upload)
                                    .await
                                {
                                    log::error!("error uploading data, failed to upload {:?} readings on this attempt: {}, collector: {:?}", collectors_len - idx, err, collector_key);
                                    break;
                                }
                                current_readings_size = 0;
                                readings_to_upload.push(next_reading);
                            } else {
                                current_readings_size += next_reading.encoded_len();
                                readings_to_upload.push(next_reading);
                            }
                        } else {
                            break;
                        }
                    }
                    if !readings_to_upload.is_empty() {
                        // if we can't upload, don't try again until after the next sleep
                        if let Err(err) = self
                            .upload_data(app_client, &collector_key, readings_to_upload)
                            .await
                        {
                            log::error!("error uploading data, failed to upload {:?} readings on this attempt: {}, collector: {:?}", collectors_len - idx, err, collector_key);
                        }
                    }
                } else {
                    log::warn!(
                        "no readings captured for collector {:?}, skipping upload",
                        collector_key
                    );
                }
            }
        }
        let _ = send_system_event(SystemEvent::DeepSleep(Some(self.sync_interval)), true).await;
        Ok(())
    }

    pub fn from_robot_and_config(
        robot: &LocalRobot,
        cfg: &RobotConfig,
        robot_start_time: Instant,
    ) -> Result<Self, DataManagerError> {
        if let Some(cfg) = get_data_service_config(cfg)? {
            let (collectors, sync_interval, part_id) =
                get_collectors_and_settings_from_robot_and_config(robot, &cfg)?;
            match sync_interval {
                None => Err(DataManagerError::MissingSyncInterval),
                Some(sync_interval) => {
                    // we validate here that the number of seconds configured between collections is
                    // the same as the interval between data syncs (this is all we can currently support
                    // and raising an error here avoids confusion)
                    if collectors.iter().any(|c| {
                        // due to discrepancies introduced by float division, we test that the difference
                        // between the collection and sync intervals is less than 0.001 seconds rather
                        // than checking for strict equality
                        (c.time_interval().as_secs_f32() - sync_interval.as_secs_f32()).abs()
                            > 0.001
                    }) {
                        Err(DataManagerError::CollectionSyncIntervalUnequal(
                            sync_interval.as_secs(),
                        ))
                    } else {
                        Ok(Self {
                            sync_interval,
                            collectors,
                            robot_start_time,
                            part_id,
                        })
                    }
                }
            }
        } else {
            Err(DataManagerError::ConfigError)
        }
    }
}

impl PeriodicAppClientTask for DataCollectAndSyncTask {
    fn name(&self) -> &str {
        "DataCollectAndSync"
    }
    fn get_default_period(&self) -> Duration {
        Duration::from_secs(0)
    }
    fn invoke<'b, 'a: 'b>(
        &'a self,
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
    use std::sync::{Arc, LazyLock, Mutex};
    use std::time::{Duration, Instant};

    use bytes::BytesMut;
    use prost::Message;
    use ringbuf::{LocalRb, Rb};

    use super::DataManager;
    use crate::common::data_collector::DataCollectionError;
    use crate::common::data_store::{DataStoreReader, WriteMode};
    use crate::common::{
        data_collector::{
            CollectionMethod, DataCollector, ResourceMethodKey, DEFAULT_CACHE_SIZE_KB,
        },
        data_store::{DataStore, DataStoreError},
        robot::ResourceType,
        sensor::{
            GenericReadingsResult, Readings, Sensor, SensorError, SensorResult, SensorT,
            TypedReadingsResult,
        },
    };
    use crate::google::protobuf::value::Kind;
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

    struct NoOpReader {}

    impl DataStoreReader for NoOpReader {
        fn read_next_message(&mut self) -> Result<BytesMut, DataStoreError> {
            Err(DataStoreError::Unimplemented)
        }
        fn messages_remaining(&self) -> Result<usize, DataStoreError> {
            Ok(1)
        }
        fn flush(self) {}
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
        fn from_resource_method_settings(
            _collector_settings: Vec<(ResourceMethodKey, usize)>,
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
            (DEFAULT_CACHE_SIZE_KB * 1000.0) as usize,
        );
        assert!(data_coll_1.is_ok());
        let data_coll_1 = data_coll_1.unwrap();

        let resource_2 = ResourceType::Sensor(Arc::new(Mutex::new(TestSensor {})));
        let data_coll_2 = DataCollector::new(
            "r2".to_string(),
            resource_2,
            CollectionMethod::Readings,
            50.0,
            (DEFAULT_CACHE_SIZE_KB * 1000.0) as usize,
        );
        assert!(data_coll_2.is_ok());
        let data_coll_2 = data_coll_2.unwrap();

        let resource_3 = ResourceType::Sensor(Arc::new(Mutex::new(TestSensor {})));
        let data_coll_3 = DataCollector::new(
            "r2".to_string(),
            resource_3,
            CollectionMethod::Readings,
            10.0,
            (DEFAULT_CACHE_SIZE_KB * 1000.0) as usize,
        );
        assert!(data_coll_3.is_ok());
        let data_coll_3 = data_coll_3.unwrap();

        let data_colls = vec![data_coll_1, data_coll_2, data_coll_3];
        let store = NoOpStore {};

        let data_manager = DataManager::new(
            data_colls,
            store,
            Some(Duration::from_millis(30)),
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
        let robot_start_time = Instant::now();
        let resource_1 = ResourceType::Sensor(Arc::new(Mutex::new(TestSensor {})));
        let data_coll_1 = DataCollector::new(
            "r1".to_string(),
            resource_1,
            CollectionMethod::Readings,
            10.0,
            (DEFAULT_CACHE_SIZE_KB * 1000.0) as usize,
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
            (DEFAULT_CACHE_SIZE_KB * 1000.0) as usize,
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
            (DEFAULT_CACHE_SIZE_KB * 1000.0) as usize,
        );
        assert!(data_coll_3.is_ok());
        let data_coll_3 = data_coll_3.unwrap();

        let data_colls = vec![data_coll_1, data_coll_2, data_coll_3];
        let store = NoOpStore {};

        let data_manager = DataManager::new(
            data_colls,
            store,
            Some(Duration::from_millis(30)),
            "1".to_string(),
        );
        assert!(data_manager.is_ok());
        let mut data_manager = data_manager.unwrap();

        let sensor_data = data_manager.collect_readings_for_interval(100, robot_start_time);
        assert!(sensor_data.is_ok());
        let sensor_data = sensor_data.unwrap();
        let sensor_data: Vec<(ResourceMethodKey, Vec<SensorData>)> = sensor_data
            .into_iter()
            .try_fold(vec![], |mut out, val| {
                out.push((val.0, val.1?));
                Ok::<Vec<(ResourceMethodKey, Vec<SensorData>)>, DataCollectionError>(out)
            })
            .unwrap();

        assert_eq!(sensor_data.len(), 2);

        assert_eq!(sensor_data[0].0, method_key_1);
        assert!(sensor_data[0].1[0].data.is_some());
        let data = sensor_data[0].1[0].data.clone().unwrap();
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
        assert!(sensor_data[1].1[0].data.is_some());
        let data = sensor_data[1].1[0].data.clone().unwrap();
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
        let robot_start_time = Instant::now();
        let resource_1 = ResourceType::Sensor(Arc::new(Mutex::new(TestSensorFailure {})));
        let data_coll_1 = DataCollector::new(
            "r1".to_string(),
            resource_1,
            CollectionMethod::Readings,
            10.0,
            (DEFAULT_CACHE_SIZE_KB * 1000.0) as usize,
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
            (DEFAULT_CACHE_SIZE_KB * 1000.0) as usize,
        );
        assert!(data_coll_3.is_ok());
        let data_coll_3 = data_coll_3.unwrap();

        let data_manager = DataManager::new(
            vec![data_coll_1, data_coll_3],
            store,
            Some(Duration::from_millis(30)),
            "1".to_string(),
        );
        assert!(data_manager.is_ok());
        let mut data_manager = data_manager.unwrap();

        let readings = data_manager
            .collect_readings_for_interval(100, robot_start_time)
            .unwrap();
        let readings: Result<Vec<(ResourceMethodKey, Vec<SensorData>)>, DataCollectionError> =
            readings.into_iter().try_fold(vec![], |mut out, val| {
                out.push((val.0, val.1?));
                Ok::<Vec<(ResourceMethodKey, Vec<SensorData>)>, DataCollectionError>(out)
            });
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

    static READ_MESSAGES: LazyLock<Mutex<Vec<SensorData>>> = LazyLock::new(|| Mutex::new(vec![]));

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
        fn messages_remaining(&self) -> Result<usize, DataStoreError> {
            Ok(self.store.borrow().len())
        }
        fn flush(self) {}
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
        fn from_resource_method_settings(
            _collector_settings: Vec<(ResourceMethodKey, usize)>,
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

    fn read_messages_for_collector(
        reader: &mut impl DataStoreReader,
    ) -> Result<Vec<SensorData>, DataStoreError> {
        let mut raw_messages: Vec<BytesMut> = vec![];
        loop {
            let next_message = reader.read_next_message()?;
            if next_message.is_empty() {
                break;
            }
            raw_messages.push(next_message);
        }
        let data: Result<Vec<SensorData>, prost::DecodeError> =
            raw_messages.into_iter().map(SensorData::decode).collect();
        Ok(data?)
    }

    #[test_log::test]
    fn test_reader() {
        let robot_start_time = Instant::now();
        let resource_1 = ResourceType::Sensor(Arc::new(Mutex::new(TestSensor {})));
        let resource_2 = ResourceType::Sensor(Arc::new(Mutex::new(TestSensor2 {})));

        let data_coll_1 = DataCollector::new(
            "r1".to_string(),
            resource_1,
            CollectionMethod::Readings,
            50.0,
            (DEFAULT_CACHE_SIZE_KB * 1000.0) as usize,
        );
        assert!(data_coll_1.is_ok());
        let data_coll_1 = data_coll_1.unwrap();
        let coll_key_1 = data_coll_1.resource_method_key();

        let data_coll_2 = DataCollector::new(
            "r2".to_string(),
            resource_2,
            CollectionMethod::Readings,
            20.0,
            (DEFAULT_CACHE_SIZE_KB * 1000.0) as usize,
        );
        assert!(data_coll_2.is_ok());
        let data_coll_2 = data_coll_2.unwrap();
        let coll_key_2 = data_coll_2.resource_method_key();

        let manager = DataManager::new(
            vec![data_coll_1, data_coll_2],
            ReadSavingStore::new(),
            Some(Duration::from_millis(65)),
            "boop".to_string(),
        );
        assert!(manager.is_ok());
        let mut manager = manager.unwrap();

        let sync_task = manager.get_sync_task(robot_start_time);
        assert!(sync_task.is_some());
        let mut sync_task = sync_task.unwrap();

        async_io::block_on(async move {
            let store_lock = sync_task.get_store_lock().await;
            let reader_1 = store_lock.get_reader(&coll_key_1);
            assert!(reader_1.is_ok());
            let mut reader_1 = reader_1.unwrap();
            let reader_2 = store_lock.get_reader(&coll_key_2);
            assert!(reader_2.is_ok());
            let mut reader_2 = reader_2.unwrap();
            std::mem::drop(store_lock);
            for i in 0..7 {
                if (i == 3) || (i == 6) {
                    let res = read_messages_for_collector(&mut reader_1);
                    assert!(res.is_ok());
                    let res = read_messages_for_collector(&mut reader_2);
                    assert!(res.is_ok());
                }
                assert!(manager
                    .collect_data_inner(i, robot_start_time)
                    .await
                    .is_ok())
            }

            let expected_data: Vec<f64> = vec![
                42.42, 42.42, 42.42, 24.24, 24.24, 42.42, 42.42, 42.42, 24.24,
            ];
            let read_data = get_read_values();
            assert_eq!(read_data, expected_data);
        });
    }
}
