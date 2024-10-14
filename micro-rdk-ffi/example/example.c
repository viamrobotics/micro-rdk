#include <micrordk.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

struct my_generic_sensor_A {
  int32_t an_int;
  int32_t *array;
  int32_t array_len;
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
  sensorA->an_int_from_config = my_int;

  // get the length of "my list" attribute first
  int32_t vec_len = -1;
  viam_code len_ret = config_get_i32_vec_len(ctx, "my_list", &vec_len);
  if (len_ret != VIAM_OK) {
    if (len_ret == VIAM_KEY_NOT_FOUND) {
      printf("my_list not found\r\n");
    }
    if (len_ret == VIAM_INVALID_ARG) {
      printf("invalid arg for getting length of my_list\r\n");
    }
    printf("defaulting to NULL\r\n");
    return len_ret;
  }

  // if we succeed in getting the length, we can proceed to extract
  // the vector behind the my_list attribute and store it in the sensor
  if (vec_len > 0) {
    int32_t *vec = malloc(sizeof(int32_t) * (size_t)vec_len);
    viam_code vec_ret = config_get_i32_vec(ctx, "my_list", vec);
    if (vec_ret != VIAM_OK) {
      if (vec_ret == VIAM_KEY_NOT_FOUND) {
        printf("my_list not found despite non-zero length\r\n");
      }
      if (vec_ret == VIAM_INVALID_ARG) {
        printf("invalid arg for my_list\r\n");
      }
      printf("defaulting to NULL\r\n");
      return vec_ret;
    }
    sensorA->array = vec;
  }

  sensorA->array_len = vec_len;

  *out = sensorA;
  
  return VIAM_OK;
}

int get_readings_my_generic_sensorA(struct get_readings_context *ctx, void* data) {
  struct my_generic_sensor_A *sensorA = data;

  get_readings_add_binary_blob(ctx, "an_int", (uint8_t*)&sensorA->an_int, sizeof(sensorA->an_int));
  get_readings_add_binary_blob(ctx, "an_int_from_config", (uint8_t*)&sensorA->an_int_from_config, sizeof(sensorA->an_int_from_config));

  return VIAM_OK;
}


struct my_generic_sensor_B {
  char *a_string;
};

int config_my_generic_sensor_B(struct config_context *ctx, void *user_data,
                               void **out) {
  char *p = NULL;
  viam_code ret = config_get_string(ctx, "my_str", &p);
  
  struct my_generic_sensor_B *sensor_B = malloc(sizeof(struct my_generic_sensor_B));
  
  if (ret == VIAM_OK) {
    char *msg = malloc(strlen(p));
    strcpy(msg, p);
    sensor_B->a_string = msg;
    config_free_string(ctx, p);
  } else {
    p = "the default string";
    char *msg = malloc(strlen(p));
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

int main() {

  struct viam_server_context *viam_ctx = init_viam_server_context();

  struct generic_c_sensor_config *config_A = generic_c_sensor_config_new();
  generic_c_sensor_config_set_user_data(config_A, NULL);
  generic_c_sensor_config_set_config_callback(config_A,
                                              config_my_generic_sensor_A);
  generic_c_sensor_config_set_readings_callback(config_A, get_readings_my_generic_sensorA);
  viam_code ret =
    viam_server_register_c_generic_sensor(viam_ctx, "sensorA", config_A);

  if (ret != VIAM_OK) {
    printf("couldn't register sensorA model cause : %i", ret);
    return EXIT_FAILURE;
  }

  struct generic_c_sensor_config *config_B = generic_c_sensor_config_new();
  generic_c_sensor_config_set_user_data(config_B, NULL);
  generic_c_sensor_config_set_config_callback(config_B,
                                              config_my_generic_sensor_B);
  generic_c_sensor_config_set_readings_callback(config_B, get_readings_my_generic_sensorB);
  ret = viam_server_register_c_generic_sensor(viam_ctx, "sensorB", config_B);

  if (ret != VIAM_OK) {
    printf("couldn't register sensorB model cause : %i", ret);
    return EXIT_FAILURE; 
  }

  printf("starting viam server\r\n");

  ret = viam_server_start(viam_ctx);

  if (ret != VIAM_OK) {
    printf("viam server returned %i", ret);
    return EXIT_FAILURE;
  }
  
  return EXIT_SUCCESS;
}
