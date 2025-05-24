/* helper macro for stack manipulation */

.macro push rx
	st \rx,r3,0
	sub r3,r3,1
.endm

.macro pop rx
	add r3,r3,1
	ld \rx,r3,0
.endm

/* call a subroutine placing the instruction after the jump as a return address */
.macro call subroutine
	.set return,(.+16)
	move r1, return
	push r1
	jump \subroutine
.endm


/* when returning from a subroutine called with call a user should make sure the next item
on the stack is the return address */
.macro ret
	pop r1
	jump r1
.endm
