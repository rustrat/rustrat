.code
rdrand_fn PROC
; On Windows, callees have to preserve rbx
    push rbx
; Pointer to buffer for random data is stored in rcx, move to r8 as cpuid will write to rcx
    mov r8, rcx

; Used CPUID to check whether rdrand is supported
    mov rax, 1
    cpuid
    bt rcx, 1Eh
    jnc no_rdrand

; Attempt to get a random number 48 times before failing
    mov rcx, 30h
do_rdrand_1:
    rdrand r9
    jc rdrand_1_end
    dec rcx
    jnz do_rdrand_1

rdrand_1_end:
    mov [r8], r9
    mov rcx, 30h
do_rdrand_2:
    rdrand r9
    jc rdrand_2_end
    dec rcx
    jnz do_rdrand_2

rdrand_2_end:
    mov [r8+8], r9
    mov rax, 1
    jmp rdrand_end

; We return 0 to signal that we were unable to generate a random number
no_rdrand:
    xor rax, rax

rdrand_end:
    pop rbx
    ret

rdrand_fn ENDP

rdtsc_fn PROC
    rdtsc
    shl rdx, 20h
    or rax, rdx
    mov [rcx], rax
    ret
rdtsc_fn ENDP

copy_tib PROC
; This could quite possibly be implemented in a more performant manner, but I don't really care right now, it is fast enough and will not be called often
    xor rax, rax

copy_loop:
    mov rdx, GS:[rax]
    mov [rcx], rdx
; Mixing add and lea, because why not
    add rax, 8
    lea rcx, [rcx+8]
    cmp rax, 68h
    jle copy_loop

    ret
copy_tib ENDP

END