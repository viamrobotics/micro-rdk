	#include "sdkconfig.h"
	#include "soc/rtc_cntl_reg.h"
	#include "soc/rtc_io_reg.h"
	#include "soc/soc_ulp.h"
	#include "soc/sens_reg.h"
	#include "soc/rtc_i2c_reg.h"
	#include "stack.S"

	.set BME280_ADDR , 0x76
	.set BME280_CHIP_ID, 0xD0
	.set BME280_CTRL_MEAS, 0xF4
	.set BME280_CTRL_HUM, 0xF2
	.set BME280_CTRL_CONFIG, 0xF5
	.set BME280_DATA_REG_BEGIN, 0xF7
	.set BME280_MEAS_READ_CNT, 8
	.set BME280_STATUS, 0xF3
	.set BME280_MEAS_STATUS_FLAG, 0x8

	.bss

/* config variables */
	.global ulp_ctrl_meas
ulp_ctrl_meas:
	.long 0
	.global ulp_ctrl_hum
ulp_ctrl_hum:
	.long 0
	.global ulp_ctrl_config
ulp_ctrl_config:
	.long 0
/* number of samples to read before waking up the processor */
	.global sample
sample:
	.long 0
/* offset for data storage in RTC slow memory (in word unit) */
	.global data_offset
data_offset:
	.long 0
stack:
	.skip 48
/* the stack pointer (r3 is used as stack pointer) */
	.global stackEnd
stackEnd:
	.long 0

	.text
	.global entry
entry:
/* initialize the stack pointer */
	move r3,stackEnd

/* potentially check r0 == 0 and if not it's not a BME280 */
	call BM280_read_id
	call BM280_config
	call BM280_wait_meas_done
	call BM280_read_meas
/* move the data pointer */
	move r2, data_offset
	ld r0,r2,0
	add r0,r0,8
	st r0,r2,0
/* if sample == 0 we wake the chip */
	move r2,sample
	ld r0,r2,0
	sub r0,r0,1
	st r0,r2,0
	jumpr next,0,gt
can_wake:
	READ_RTC_FIELD(RTC_CNTL_LOW_POWER_ST_REG, RTC_CNTL_RDY_FOR_WAKEUP)
	AND r0, r0, 1
	JUMP can_wake, eq
	wake
	WRITE_RTC_FIELD(RTC_CNTL_STATE0_REG,RTC_CNTL_ULP_CP_SLP_TIMER_EN,0)
next:
	halt

BM280_wait_meas_done:
	move r1, BME280_ADDR
	push r1
	move r1, BME280_STATUS
	push r1
	move r1, 1
	push r1
	push r3
	call i2c_read_registers
	pop r0
	add r3,r3,3
	and r0,r0,BME280_MEAS_STATUS_FLAG
	jumpr BM280_wait_meas_done,0,gt
	ret

BM280_read_id:
	move r1, BME280_ADDR
	push r1
	move r1, BME280_CHIP_ID
	push r1
	move r1, 1
	push r1
	push r3
	call i2c_read_registers
	pop r0
	sub r0,r0,0x60
	add r3,r3,3
	ret
BM280_read_meas:
	move r1, BME280_ADDR
	push r1
	move r1, BME280_DATA_REG_BEGIN
	push r1
	move r1, BME280_MEAS_READ_CNT
	push r1
	move r1, data_offset
	ld r1, r1,0
	push r1
	call i2c_read_registers
	add r3,r3,4
	ret

BM280_config:
	move r1, BME280_ADDR
	push r1
	move r1, BME280_CTRL_HUM
	push r1
	move r1, ulp_ctrl_hum
	push r1
	call i2c_write_register8
	add r3,r3,3

	move r1, BME280_ADDR
	push r1
	move r1, BME280_CTRL_CONFIG
	push r1
	move r1, ulp_ctrl_config
	push r1
	call i2c_write_register8
	add r3,r3,3

	move r1, BME280_ADDR
	push r1
	move r1, BME280_CTRL_MEAS
	push r1
	move r1, ulp_ctrl_meas
	push r1
	call i2c_write_register8
	add r3,r3,3

	ret
