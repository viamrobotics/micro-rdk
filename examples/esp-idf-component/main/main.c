#include "esp_event.h"
#include "esp_log.h"
#include "esp_mac.h"
#include "esp_random.h"
#include "esp_system.h"
#include "esp_wifi.h"
#include "freertos/FreeRTOS.h"
#include "freertos/event_groups.h"
#include "freertos/task.h"
#include "nvs_flash.h"
#include "sdkconfig.h"
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>


#ifdef CONFIG_MICRO_RDK_ENABLE_BUILD_LIBRARY
#include <micrordk.h>
#endif

#define EXAMPLE_ESP_MAXIMUM_RETRY CONFIG_ESP_MAXIMUM_RETRY
#define EXAMPLE_ESP_WIFI_SSID CONFIG_ESP_WIFI_SSID
#define EXAMPLE_ESP_WIFI_PASS CONFIG_ESP_WIFI_PASSWORD
#define WIFI_CONNECTED_BIT BIT0
#define WIFI_FAIL_BIT      BIT1

static const char* TAG = "micrordk-lib-example";
static EventGroupHandle_t s_wifi_event_group;


static int s_retry_num = 0;


static void event_handler(void* arg, esp_event_base_t event_base,
			  int32_t event_id, void* event_data)
{
  if (event_base == WIFI_EVENT && event_id == WIFI_EVENT_STA_START) {
    esp_wifi_connect();
  } else if (event_base == WIFI_EVENT && event_id == WIFI_EVENT_STA_DISCONNECTED) {
    if (s_retry_num < EXAMPLE_ESP_MAXIMUM_RETRY) {
      esp_wifi_connect();
      s_retry_num++;
      ESP_LOGI(TAG, "retry to connect to the AP");
    } else {
      xEventGroupSetBits(s_wifi_event_group, WIFI_FAIL_BIT);
    }
    ESP_LOGI(TAG, "connect to the AP fail");
  } else if (event_base == IP_EVENT && event_id == IP_EVENT_STA_GOT_IP) {
    ip_event_got_ip_t* event = (ip_event_got_ip_t*) event_data;
    ESP_LOGI(TAG, "got ip:" IPSTR, IP2STR(&event->ip_info.ip));
    s_retry_num = 0;
    xEventGroupSetBits(s_wifi_event_group, WIFI_CONNECTED_BIT);
  }
}

void wifi_init_sta(void)
{
  s_wifi_event_group = xEventGroupCreate();

  ESP_ERROR_CHECK(esp_netif_init());

  ESP_ERROR_CHECK(esp_event_loop_create_default());
  esp_netif_create_default_wifi_sta();

  wifi_init_config_t cfg = WIFI_INIT_CONFIG_DEFAULT();
  ESP_ERROR_CHECK(esp_wifi_init(&cfg));

  esp_event_handler_instance_t instance_any_id;
  esp_event_handler_instance_t instance_got_ip;
  ESP_ERROR_CHECK(esp_event_handler_instance_register(WIFI_EVENT,
						      ESP_EVENT_ANY_ID,
						      &event_handler,
						      NULL,
						      &instance_any_id));
  ESP_ERROR_CHECK(esp_event_handler_instance_register(IP_EVENT,
						      IP_EVENT_STA_GOT_IP,
						      &event_handler,
						      NULL,
						      &instance_got_ip));

  wifi_config_t wifi_config = {
    .sta = {
      .ssid = EXAMPLE_ESP_WIFI_SSID,
      .password = EXAMPLE_ESP_WIFI_PASS,
      /* Setting a password implies station will connect to all security modes including WEP/WPA.
       * However these modes are deprecated and not advisable to be used. Incase your Access point
       * doesn't support WPA2, these mode can be enabled by commenting below line */
      .threshold.authmode = WIFI_AUTH_WPA2_PSK,
      .sae_pwe_h2e = WPA3_SAE_PWE_BOTH,
    },
  };
  ESP_ERROR_CHECK(esp_wifi_set_mode(WIFI_MODE_STA) );
  ESP_ERROR_CHECK(esp_wifi_set_config(WIFI_IF_STA, &wifi_config) );
  ESP_ERROR_CHECK(esp_wifi_start() );

  ESP_LOGI(TAG, "wifi_init_sta finished.");

  /* Waiting until either the connection is established (WIFI_CONNECTED_BIT) or connection failed for the maximum
   * number of re-tries (WIFI_FAIL_BIT). The bits are set by event_handler() (see above) */
  EventBits_t bits = xEventGroupWaitBits(s_wifi_event_group,
					 WIFI_CONNECTED_BIT | WIFI_FAIL_BIT,
					 pdFALSE,
					 pdFALSE,
					 portMAX_DELAY);

  /* xEventGroupWaitBits() returns the bits before the call returned, hence we can test which event actually
   * happened. */
  if (bits & WIFI_CONNECTED_BIT) {
    ESP_LOGI(TAG, "connected to ap SSID:%s password:%s",
	     EXAMPLE_ESP_WIFI_SSID, EXAMPLE_ESP_WIFI_PASS);
  } else if (bits & WIFI_FAIL_BIT) {
    ESP_LOGI(TAG, "Failed to connect to SSID:%s, password:%s",
	     EXAMPLE_ESP_WIFI_SSID, EXAMPLE_ESP_WIFI_PASS);
  } else {
    ESP_LOGE(TAG, "UNEXPECTED EVENT");
  }

  /* The event will not be processed after unregister */
  ESP_ERROR_CHECK(esp_event_handler_instance_unregister(IP_EVENT, IP_EVENT_STA_GOT_IP, instance_got_ip));
  ESP_ERROR_CHECK(esp_event_handler_instance_unregister(WIFI_EVENT, ESP_EVENT_ANY_ID, instance_any_id));
  vEventGroupDelete(s_wifi_event_group);
}




#ifdef CONFIG_MICRO_RDK_ENABLE_BUILD_LIBRARY
struct sensorA_random_record{
  uint32_t id;
  uint8_t array[8];
  uint32_t timestamp_ms;
};

struct sensorA_random_record* generate_new_sensorA_random_record(uint32_t id)
{
  struct sensorA_random_record* record = malloc(sizeof(struct sensorA_random_record));
  if (record == NULL) {
    ESP_LOGE(TAG, "couldn't allocate a record");
  }
  record->id = id;
  esp_fill_random(record->array,sizeof(record->array));
  record->timestamp_ms = esp_log_timestamp();
  return record;
}

struct my_generic_sensor_A {
  int32_t an_int;
  pthread_mutex_t hash_map_lock;
  struct hashmap_cstring_ptr *hash_map;
  int32_t an_int_from_config;
};

int config_my_generic_sensor_A(struct config_context *ctx, void *user_data,
                               void **out) {
  int32_t my_int = -1;
  viam_code ret = config_get_i32(ctx, "my_int", &my_int);
  
  if (ret != VIAM_OK) {
    printf("defaulting to -1\r\n");
  }

  struct my_generic_sensor_A *sensorA = malloc(sizeof(struct my_generic_sensor_A));

  sensorA->an_int = 1234567;
  sensorA->hash_map = hashmap_cstring_ptr_new();
  sensorA->an_int_from_config = my_int;
  pthread_mutex_init(&sensorA->hash_map_lock, NULL);

  struct sensorA_random_record* r = generate_new_sensorA_random_record(1000);
  if (r != NULL){
    hashmap_cstring_ptr_insert(sensorA->hash_map, "1000", (const void*)r);
  }
  
  r = generate_new_sensorA_random_record(2000);
  if (r != NULL){
    hashmap_cstring_ptr_insert(sensorA->hash_map, "2000", (const void*)r);
  }
  
  r = generate_new_sensorA_random_record(1111);
  if (r != NULL){
    hashmap_cstring_ptr_insert(sensorA->hash_map, "1111", (const void*)r);
  }
  
  *out = sensorA;
  
  return VIAM_OK;
}

void from_hash_map_add_readings(void *reading_context, const char *key,
                                const void *data) {
  struct get_readings_context* ctx = (struct get_readings_context*)reading_context;
  struct sensorA_random_record* record = (struct sensorA_random_record*)(data);
  get_readings_add_binary_blob(ctx,key,(uint8_t*)record,sizeof(struct sensorA_random_record));
}

int get_readings_my_generic_sensorA(struct get_readings_context *ctx, void* data) {
  struct my_generic_sensor_A *sensorA = data;

  pthread_mutex_lock(&sensorA->hash_map_lock);

  hashmap_cstring_ptr_for_each_kv(sensorA->hash_map, (void*)ctx, from_hash_map_add_readings);

  get_readings_add_binary_blob(ctx, "an_int", (uint8_t*)&sensorA->an_int, sizeof(sensorA->an_int));
  get_readings_add_binary_blob(ctx, "an_int_from_config", (uint8_t*)&sensorA->an_int_from_config, sizeof(sensorA->an_int_from_config));

  pthread_mutex_unlock(&sensorA->hash_map_lock);

  return VIAM_OK;
}


struct my_generic_sensor_B {
  char *a_string;
};

int config_my_generic_sensor_B(struct config_context *ctx, void *user_data,
                               void **out) {
  char *p = NULL;
  
  struct my_generic_sensor_B *sensor_B = malloc(sizeof(struct my_generic_sensor_B));
  viam_code ret = config_get_string(ctx, "my_str", &p);

  if (ret == VIAM_OK) {
    char *msg = malloc(strlen(p)+1);
    strcpy(msg, p);
    sensor_B->a_string = msg;
    config_free_string(ctx, p);
  } else {
    p = "the default string";
    char *msg = malloc(strlen(p)+1);
    strcpy(msg, p);
    sensor_B->a_string = msg;
  }

  *out = sensor_B;
  
  return VIAM_OK;
}

int get_readings_my_generic_sensorB(struct get_readings_context *ctx, void* data) {
  struct my_generic_sensor_B *sensorB = data;

  get_readings_add_string(ctx, "string", sensorB->a_string);
  

  return VIAM_OK;
}

#endif

void app_main(void)
{
  esp_err_t f_ret = nvs_flash_init();
  if (f_ret == ESP_ERR_NVS_NO_FREE_PAGES || f_ret == ESP_ERR_NVS_NEW_VERSION_FOUND) {
    ESP_ERROR_CHECK(nvs_flash_erase());
    f_ret = nvs_flash_init();
  }
  ESP_ERROR_CHECK(f_ret);

  ESP_LOGI(TAG, "ESP_WIFI_MODE_STA");
  wifi_init_sta();

#ifdef CONFIG_MICRO_RDK_ENABLE_BUILD_LIBRARY

  struct viam_server_context *viam_ctx = init_viam_server_context();

  struct generic_c_sensor_config *config_A = generic_c_sensor_config_new();
  generic_c_sensor_config_set_user_data(config_A, NULL);
  generic_c_sensor_config_set_config_callback(config_A,
                                              config_my_generic_sensor_A);
  generic_c_sensor_config_set_readings_callback(config_A, get_readings_my_generic_sensorA);
  viam_code ret =
    viam_server_register_c_generic_sensor(viam_ctx, "sensorA", config_A);

  if (ret != VIAM_OK) {
    ESP_LOGE(TAG, "couldn't register sensorA model, error : %i", ret);
    return;
  }

  struct generic_c_sensor_config *config_B = generic_c_sensor_config_new();
  generic_c_sensor_config_set_user_data(config_B, NULL);
  generic_c_sensor_config_set_config_callback(config_B,
                                              config_my_generic_sensor_B);
  generic_c_sensor_config_set_readings_callback(config_B, get_readings_my_generic_sensorB);
  ret = viam_server_register_c_generic_sensor(viam_ctx, "sensorB", config_B);

  if (ret != VIAM_OK) {
    ESP_LOGE(TAG, "couldn't register sensorB model, error : %i", ret);
    return;
  }

  ret = viam_server_set_provisioning_manufacturer(viam_ctx, "viam-example");
  if (ret != VIAM_OK) {
    ESP_LOGE(TAG, "couldn't set manufacturer, error : %i", ret);
    return;
  }

  uint8_t mac[8];
  esp_err_t esp_err = esp_efuse_mac_get_default(mac);
  if (esp_err != ESP_OK){
    ESP_LOGE(TAG, "couldn't get default mac, error : %i", esp_err);
    return;
  }
  char model[50];
  snprintf(model, 50, "esp32-%02X%02X", mac[6],mac[7]);
  ret = viam_server_set_provisioning_model(viam_ctx, model);
  if (ret != VIAM_OK) {
    ESP_LOGE(TAG, "couldn't set model, error : %i", ret);
    return;
  }

  ret = viam_server_add_nvs_storage(viam_ctx, "nvs");
  if (ret != VIAM_OK) {
    ESP_LOGE(TAG, "couldn't set add nvs partition, error : %i", ret);
    return;
  }

  ret = viam_server_add_nvs_storage(viam_ctx, "nvs_other");
  if (ret != VIAM_OK) {
    ESP_LOGE(TAG, "couldn't set add nvs partition, error : %i", ret);
    return;
  }

  ESP_LOGI(TAG, "starting viam server\r\n");

  xTaskCreatePinnedToCore((void*)viam_server_start, "viam", CONFIG_MICRO_RDK_TASK_STACK_SIZE, viam_ctx, 6, NULL, CONFIG_MICRO_RDK_TASK_PINNED_TO_CORE_1);
#else
  ESP_LOGE(TAG, "enable MICRO_RDK_ENABLE_BUILD_LIBRARY ");
#endif

}
