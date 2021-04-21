use blake2::{Blake2s, Digest};

// rdrand_fn needs an array of 16 bytes and will fill the array with random data from rdrand. A return value of 0 means that rdrand could not provide random values. Note that some values may have changed in the array.
// rdtsc_fn needs an array of 8 bytes which will be filled with the result from rdts, which returns a 64 bit number. On x86_64 the result is returned as well, while on x86, the return value will be the 32 least significat bits (I think).
// copy_tib copies part of the TIB to the buffer. To be safe, supply an array of at least 110 bytes, as that is what will be copied on x86_64. x86 will get less data from the tib, but a 110 byte buffer should be used to keep things simple.

#[cfg(target_arch = "x86")]
extern "cdecl" {
    fn rdrand_fn(output_buffer: *mut u8) -> u32;
    fn rdtsc_fn(output_buffer: *mut u8) -> u32;
    fn copy_tib(output_buffer: *mut u8) -> u32;
}

#[cfg(target_arch = "x86_64")]
extern "C" {
    fn rdrand_fn(output_buffer: *mut u8) -> u64;
    fn rdtsc_fn(output_buffer: *mut u8) -> u64;
    fn copy_tib(output_buffer: *mut u8) -> u64;
}

pub fn get_rand_seed() -> [u8; 32] {
    // The different functions expects 16, 8, and 110 bytes, so we need a buffer of at least (16+8+110=)134 bytes.
    let mut out_buf = [0u8; 124];

    let (rdrand, remainder) = out_buf.split_at_mut(16);
    let (rdtsc, tib) = remainder.split_at_mut(8);

    unsafe {
        rdrand_fn(rdrand.as_mut_ptr());
        rdtsc_fn(rdtsc.as_mut_ptr());
        copy_tib(tib.as_mut_ptr());
    }

    // TODO possibly use something other than Blake2? Used for simplicity and because I believe at least Blake2 will not be a weak point when it comes to security (the "entropy" if rdrand is missing is a larger problem)
    let mut hasher = Blake2s::new();
    hasher.update(out_buf);
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use std::convert::TryInto;
    use std::process::id;

    use rand::rngs::StdRng;
    use rand::{RngCore, SeedableRng};

    #[test]
    fn rdrand() {
        // Provide a bit larger buffer to make sure that the function does not write outside its designated space.
        let mut buf = [0u8; 18];
        unsafe {
            // Call a bunch of times to make sure the stack does not get messed up or something like that.
            for _ in 0..100 {
                super::rdrand_fn(buf.as_mut_ptr());
            }
        }

        assert_eq!(buf[16], 0, "rdrand_fn wrote outside its buffer");
        assert_eq!(buf[17], 0, "rdrand_fn wrote outside its buffer");
    }

    #[test]
    fn rdtsc() {
        let mut buf = [0u8; 10];
        unsafe {
            // Call a bunch of times to make sure the stack does not get messed up or something like that.
            for _ in 0..100 {
                super::rdtsc_fn(buf.as_mut_ptr());
            }
        }

        assert_eq!(buf[8], 0, "rdtsc_fn wrote outside its buffer");
        assert_eq!(buf[9], 0, "rdtsc_fn wrote outside its buffer");
    }

    #[cfg(target_arch = "x86")]
    fn get_tib_pid(tib: &[u8]) -> u32 {
        u32::from_le_bytes(tib[0x20..0x24].try_into().unwrap())
    }

    #[cfg(target_arch = "x86_64")]
    fn get_tib_pid(tib: &[u8]) -> u32 {
        // Rust always returns a 32 bit number for PID. Will leave as is unless it turns out to cause any bugs.
        u64::from_le_bytes(tib[0x40..0x48].try_into().unwrap()) as u32
    }

    #[test]
    fn tib() {
        let mut buf = [0u8; 112];
        unsafe {
            // Call a bunch of times to make sure the stack does not get messed up or something like that.
            for _ in 0..100 {
                super::copy_tib(buf.as_mut_ptr());
            }
        }

        assert_eq!(buf[110], 0, "copy_tib wrote outside its buffer");
        assert_eq!(buf[111], 0, "copy_tib wrote outside its buffer");

        let tib_pid = get_tib_pid(&buf);
        assert_eq!(tib_pid, id(), "Process id parsed from TIB is incorrect.");
    }

    #[test]
    fn seed_stdrng() {
        let mut rng1 = StdRng::from_seed(super::get_rand_seed());
        let mut rng2 = StdRng::from_seed(super::get_rand_seed());

        // TODO this test is probably stupid, change with something that does not have a chance of breaking?
        assert_ne!(rng1.next_u64(), rng2.next_u64());
    }
}
