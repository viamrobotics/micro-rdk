	#include "soc/rtc_cntl_reg.h"
	#include "soc/rtc_io_reg.h"
	#include "soc/soc_ulp.h"
	#include "stack.S"

	.bss
i2c_started:
	.long 0
	.data
/* the NOP instruction will be replaced with the correct IO register address */
	.global set_SCL
set_SCL:
	NOP
	ret
	.global clear_SCL
clear_SCL:
	NOP
	ret
	.global read_SCL
read_SCL:
	NOP
	ret
	.global set_SDA
set_SDA:
	NOP
	ret
	.global clear_SDA
clear_SDA:
	NOP
	ret
	.global read_SDA
read_SDA:
	NOP
	ret

	.text
	.global i2c_read_registers
	.global i2c_write_register8

	.macro I2C_delay
	wait 50		/* if number equ 10 then clock gap is minimal 4.7us */
	.endm


i2c_start_cond:
	move r2,i2c_started
	ld r0,r2,0
	jumpr not_started,1,lt	/* if started, do a restart condition */
	call set_SDA
	I2C_delay
	call set_SCL
clock_stretch:
	call read_SCL
	jumpr clock_stretch,1,lt
	I2C_delay
not_started:
	call clear_SDA
	I2C_delay
	call clear_SCL
	move r0,1
	st r0,r2,0
	ret

i2c_stop_cond:
	call clear_SDA
	I2C_delay
	call set_SCL
clock_stretch_stop:
	call read_SCL
	jumpr clock_stretch_stop,1,lt
	I2C_delay
	call set_SDA
	I2C_delay
	move r2,i2c_started
	move r0,0
	st r0,r2,0
	ret

i2c_write_bit:
	jumpr bit0,1,lt
	call set_SDA
	jump bit1
bit0:
	call clear_SDA
bit1:
	I2C_delay
	call set_SCL
	I2C_delay
clock_stretch_write:
	call read_SCL
	jumpr clock_stretch_write,1,lt
	call clear_SCL
	ret

i2c_read_bit:
	call set_SDA
	I2C_delay
	call set_SCL
clock_stretch_read:
	call read_SCL
	jumpr clock_stretch_read,1,lt
	I2C_delay
	call read_SDA
	call clear_SCL
	ret

i2c_write_byte:
	stage_rst
next_bit:
	and r0,r2,0x80
	call i2c_write_bit
	lsh r2,r2,1
	stage_inc 1
	jumps next_bit,8,lt
	call i2c_read_bit
	ret

i2c_read_byte:
	push r2
	move r2,0
	stage_rst
next_bit_read:
	call i2c_read_bit
	lsh r2,r2,1
	or r2,r2,r0
	stage_inc 1
	jumps next_bit_read,8,lt
	pop r0
	call i2c_write_bit
	move r0, r2
	ret

/* equivalent to uint16_t i2c_read_register8(uint8_t dev_addr, uint8_t reg_addr, uint16_t n uint8_t *data);
*/
i2c_read_registers:
	move r1, 0 // cnt
	push r1

	call i2c_start_cond
	ld r2, r3, 24 // load device address
	lsh r2,r2,1
	call i2c_write_byte
	jumpr fail,1,ge
	ld r2,r3,20 // load register address
	call i2c_write_byte
	jumpr fail,1,ge

	call i2c_start_cond
	ld r2,r3,24 // load device address
	lsh r2,r2,1
	or r2,r2,1
	call i2c_write_byte
	jumpr fail,1,ge
	ld r0,r3,16 // load n
i2c_read_one_byte:
	move r2,0
	sub r0,r0,1 // doing one read
	st r0,r3,16
	jumpr i2c_read_continue,0,gt
	move r2,1
i2c_read_continue:
	call i2c_read_byte
	ld r2, r3, 12 // load data address
	pop r1
	add r2,r2,r1 // add offset
	st r0, r2, 0
	add r1,r1,1
	push r1
	ld r0,r3,16 // load n
	jumpr i2c_read_one_byte,0,gt

	call i2c_stop_cond
	pop r1
	move r0,0
	ret

/* equivalent to uint16_t i2c_write_register8(uint8_t dev_addr, uint8_t reg_addr, uint8_t data); */
i2c_write_register8:
	call i2c_start_cond
	ld r2, r3, 16
	lsh r2,r2,1
	call i2c_write_byte
	jumpr fail,1,ge

	ld r2,r3,12 // Register
	call i2c_write_byte
	jumpr fail,1,ge

	ld r2,r3,8
	call i2c_write_byte
	jumpr fail,1,ge
	call i2c_stop_cond
	ret
fail:
	ret
