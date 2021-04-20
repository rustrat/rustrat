.MODEL FLAT, C
.code
rdrand_fn PROC
; Using cdelc, callees have to preserve ebx
    push ebx
    push [esp-4]

; Used CPUID to check whether rdrand is supported
    mov eax, 1
    cpuid
    bt ecx, 1Eh
    jnc no_rdrand
    pop edx

; Attempt to get a random number 48 times before failing
    mov ecx, 30h
do_rdrand_1:
    rdrand eax
    jc rdrand_1_end
    dec ecx
    jnz do_rdrand_1

rdrand_1_end:
    mov [edx], eax
    mov ecx, 30h

do_rdrand_2:
    rdrand eax
    jc rdrand_2_end
    dec ecx
    jnz do_rdrand_2

rdrand_2_end:
    mov [edx+4], eax
    mov ecx, 30h

do_rdrand_3:
    rdrand eax
    jc rdrand_3_end
    dec ecx
    jnz do_rdrand_3

rdrand_3_end:
    mov [edx+8], eax
    mov ecx, 30h

do_rdrand_4:
    rdrand eax
    jc rdrand_4_end
    dec ecx
    jnz do_rdrand_4

rdrand_4_end:
    mov [edx+12], eax
    mov eax, 1
    jmp rdrand_end

; We return 0 to signal that we were unable to generate a random number
; This means that we cannot distinguish between a random 0 and failing, but that is a small price to pay
no_rdrand:
    xor eax, eax

rdrand_end:
    pop ebx
    ret

rdrand_fn ENDP

rdtsc_fn PROC
    mov ecx, [esp+4]
    rdtsc
    mov [ecx], eax
    mov [ecx+4], edx
    ret
rdtsc_fn ENDP

copy_tib PROC
; This could quite possibly be implemented in a more performant manner, but I don't really care right now, it is fast enough and will not be called often
    mov ecx, [esp+4]
    xor eax, eax

copy_loop:
    ASSUME FS:NOTHING
    mov edx, FS:[eax]
    ASSUME FS:ERROR
    mov [ecx], edx
; Mixing add and lea, because why not
    add eax, 4
    lea ecx, [ecx+4]
    cmp eax, 34h 
    jle copy_loop

    ret
copy_tib ENDP
END