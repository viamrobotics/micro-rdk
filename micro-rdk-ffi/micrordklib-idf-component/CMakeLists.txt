idf_component_register(INCLUDE_DIRS "${CMAKE_CURRENT_SOURCE_DIR}")
idf_build_set_property(INCLUDE_DIRECTORIES "${CMAKE_CURRENT_BINARY_DIR}/assets" APPEND)

set(LIBMICRORDK_VERSION v0.2.8)
set(LIBMICRORDK_URL https://github.com/viamrobotics/micro-rdk/releases/download/${LIBMICRORDK_VERSION}/micro-rdk-lib.zip)
set(LIBMICRORDK_PATH ${CMAKE_BINARY_DIR}/import/micro-rdk-lib.zip)

if((NOT EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/libmicrordk.a" AND NOT IS_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}/libmicrordk.a") OR(NOT EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/micrordk.h" AND NOT IS_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}/micrordk.h"))
  file(DOWNLOAD ${LIBMICRORDK_URL} ${LIBMICRORDK_PATH} STATUS LIBMICRORDK_DOWNLOAD_STATUS)
  list(GET LIBMICRORDK_DOWNLOAD_STATUS 0 LIBMICRORDK_DOWNLOAD_STATUS_NO)
  if(LIBMICRORDK_DOWNLOAD_STATUS_NO)
    message( FATAL_ERROR "Cannot download Micro-RDK ${LIBMICRORDK_VERSION} check if the version is valid" )
  else()
    add_prebuilt_library(micro_rdk_ffi
      "${CMAKE_CURRENT_BINARY_DIR}/assets/libmicrordk.a"
      PRIV_REQUIRES "esp32" "pthread" "driver" "vfs" "esp_adc_cal" "esp-tls" "mdns"
    )
    add_custom_target(build-time-make-directory ALL
      COMMAND ${CMAKE_COMMAND} -E make_directory assets)
    add_custom_command(OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/assets/libmicrordk.a"
      COMMAND ${CMAKE_COMMAND} -E tar xzf ${LIBMICRORDK_PATH}
      WORKING_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/assets"
      COMMENT Extract ${LIBMICRORDK_PATH}/assets
      BYPRODUCT "${CMAKE_CURRENT_BINARY_DIR}/micrordk-lib/libmicrordk.a"
      VERBATIM
    )
    add_custom_target(libmicrordk.a DEPENDS "${CMAKE_CURRENT_BINARY_DIR}/assets/libmicrordk.a")
  endif()
else()
  add_prebuilt_library(micro_rdk_ffi
    "${CMAKE_CURRENT_SOURCE_DIR}/libmicrordk.a"
    PRIV_REQUIRES "esp32" "pthread" "driver" "vfs" "esp_adc_cal" "esp-tls" "mdns"
  )
endif()

target_link_libraries(${COMPONENT_LIB}
    INTERFACE micro_rdk_ffi
)
