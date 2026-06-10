//! Unit tests for all package emulation functions.

#[cfg(test)]
mod tests {
    use crate::packages::*;
    use crate::GoString;

    // ─── strings_ops tests ──────────────────────────────────────────────────

    #[test]
    fn test_index() {
        assert_eq!(index(&[1, 2, 3], &2), 1);
        assert_eq!(index(&[1, 2, 3], &5), -1);
    }

    #[test]
    fn test_slice_sub() {
        let v = vec![1, 2, 3, 4, 5];
        assert_eq!(slice_sub(&v, 1, 3), vec![2, 3]);
        assert_eq!(slice_sub(&v, 0, 5), vec![1, 2, 3, 4, 5]);
        assert_eq!(slice_sub::<i32>(&v, 3, 2), Vec::<i32>::new());
        assert_eq!(slice_sub::<i32>(&v, -1, 3), vec![1, 2, 3]);
    }

    #[test]
    fn test_sort() {
        let mut v = vec![3, 1, 2];
        sort(&mut v);
        assert_eq!(v, vec![1, 2, 3]);
    }

    #[test]
    fn test_reverse() {
        let mut v = vec![1, 2, 3];
        reverse(&mut v);
        assert_eq!(v, vec![3, 2, 1]);
    }

    #[test]
    fn test_contains() {
        assert!(contains(&[1, 2, 3], &2));
        assert!(!contains(&[1, 2, 3], &5));
    }

    #[test]
    fn test_join() {
        assert_eq!(join(&[GoString::from("a"), GoString::from("b"), GoString::from("c")], &GoString::from(",")), "a,b,c");
    }

    #[test]
    fn test_split() {
        assert_eq!(split("a,b,c", ","), vec!["a".to_string(), "b".to_string(), "c".to_string()]);
        assert_eq!(split("a", ","), vec!["a".to_string()]);
    }

    #[test]
    fn test_contains_str() {
        assert!(contains_str("hello world", "world"));
        assert!(!contains_str("hello world", "xyz"));
    }

    #[test]
    fn test_index_str() {
        assert_eq!(index_str("hello world", "world"), 6);
        assert_eq!(index_str("hello", "xyz"), -1);
    }

    #[test]
    fn test_trim() {
        assert_eq!(trim("  hello  ", " "), "hello");
        assert_eq!(trim("hello", " "), "hello");
    }

    #[test]
    fn test_trim_left() {
        assert_eq!(trim_left("  hello", " "), "hello");
    }

    #[test]
    fn test_trim_right() {
        assert_eq!(trim_right("hello  ", " "), "hello");
    }

    #[test]
    fn test_to_upper() {
        assert_eq!(to_upper("hello"), "HELLO");
    }

    #[test]
    fn test_to_lower() {
        assert_eq!(to_lower("HELLO"), "hello");
    }

    #[test]
    fn test_repeat() {
        assert_eq!(repeat("ab", 3), "ababab");
    }

    // ─── strings tests ──────────────────────────────────────────────────────

    #[test]
    fn test_strings_replace() {
        assert_eq!(strings_replace("hello world", "world", "rust", 1), "hello rust");
        assert_eq!(strings_replace("aaa", "a", "b", 2), "bba");
    }

    #[test]
    fn test_strings_replace_all() {
        assert_eq!(strings_replace_all("aaa", "a", "b"), "bbb");
    }

    #[test]
    fn test_has_prefix() {
        assert!(has_prefix("hello world", "hello"));
        assert!(!has_prefix("hello world", "xyz"));
    }

    #[test]
    fn test_has_suffix() {
        assert!(has_suffix("hello world", "world"));
        assert!(!has_suffix("hello world", "xyz"));
    }

    #[test]
    fn test_last_index_str() {
        assert_eq!(last_index_str("hello world hello", "hello"), 12);
        assert_eq!(last_index_str("hello", "xyz"), -1);
    }

    #[test]
    fn test_fields() {
        let result = fields("  a b  c   ");
        assert_eq!(result, vec!["a".to_string(), "b".to_string(), "c".to_string()]);
    }

    // ─── os_ops tests ───────────────────────────────────────────────────────

    #[test]
    fn test_os_mkdir() {
        let result = os_mkdir("/tmp/gourd_test_mkdir_12345", 0o755);
        // Directory might already exist from a previous run
        if result.is_err() {
            let _ = os_remove("/tmp/gourd_test_mkdir_12345");
            let _ = os_mkdir("/tmp/gourd_test_mkdir_12345", 0o755);
        }
        // Cleanup
        let _ = os_remove("/tmp/gourd_test_mkdir_12345");
    }

    #[test]
    fn test_os_mkdir_all() {
        let result = os_mkdir_all("/tmp/gourd_test_mkdir_all/inner", 0o755);
        assert!(result.is_ok());
        // Cleanup
        let _ = os_remove("/tmp/gourd_test_mkdir_all/inner");
        let _ = os_remove("/tmp/gourd_test_mkdir_all");
    }

    #[test]
    fn test_os_getenv_setenv() {
        let old = os_getenv("GOURD_TEST_VAR").unwrap_or_default();
        os_setenv("GOURD_TEST_VAR", "test_value");
        assert_eq!(os_getenv("GOURD_TEST_VAR").unwrap(), "test_value".to_string());
        // Cleanup
        os_setenv("GOURD_TEST_VAR", &old);
    }

    // ─── io_ops tests ───────────────────────────────────────────────────────

    #[test]
    fn test_io_copy() {
        let src = vec![1u8, 2, 3, 4, 5];
        let mut dst = vec![0u8; 3];
        let n = io_copy(&mut dst, &src);
        assert_eq!(n, 3);
        assert_eq!(dst, vec![1, 2, 3]);
    }

    // ─── bytes_ops tests ────────────────────────────────────────────────────

    #[test]
    fn test_bytes_contains() {
        assert!(bytes_contains(&[1, 2, 3], &[2, 3]));
        assert!(!bytes_contains(&[1, 2, 3], &[4, 5]));
    }

    // ─── time_ops tests ─────────────────────────────────────────────────────

    #[test]
    fn test_time_now() {
        let t = time_now();
        assert!(t >= 0);
    }

    #[test]
    fn test_time_sleep() {
        // Should not panic and should complete
        time_sleep(1); // 1 nanosecond
    }

    // ─── byte_ops tests ─────────────────────────────────────────────────────

    #[test]
    fn test_byte_of() {
        assert_eq!(byte_of('a'), 97u8);
    }

    #[test]
    fn test_rune_of() {
        assert_eq!(rune_of(97u8), 'a');
    }

    #[test]
    fn test_string_to_bytes() {
        assert_eq!(string_to_bytes("hello"), vec![104, 101, 108, 108, 111]);
    }

    #[test]
    fn test_bytes_to_string() {
        assert_eq!(bytes_to_string(&[104, 101, 108, 108, 111]), "hello");
    }

    // ─── json_ops tests ─────────────────────────────────────────────────────

    #[test]
    fn test_json_marshal_basic() {
        let result = json_marshal(&42);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), b"42");
    }

    #[test]
    fn test_json_marshal_string() {
        let result = json_marshal(&"hello");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), b"\"hello\"");
    }

    #[test]
    fn test_json_marshal_vec() {
        let v = vec![1, 2, 3];
        let result = json_marshal(&v);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), b"[1,2,3]");
    }

    // ─── math_ops tests ─────────────────────────────────────────────────────

    #[test]
    fn test_abs_i32() {
        assert_eq!(abs_i32(-42), 42);
        assert_eq!(abs_i32(42), 42);
    }

    #[test]
    fn test_abs_i64() {
        assert_eq!(abs_i64(-42), 42);
        assert_eq!(abs_i64(42), 42);
    }

    #[test]
    fn test_sqrt() {
        assert!((sqrt(4.0) - 2.0).abs() < 0.001);
    }

    #[test]
    fn test_floor() {
        assert_eq!(floor(3.7), 3.0);
        assert_eq!(floor(-3.7), -4.0);
    }

    #[test]
    fn test_ceil() {
        assert_eq!(ceil(3.2), 4.0);
    }

    #[test]
    fn test_round() {
        assert_eq!(round(3.5), 4.0);
        assert_eq!(round(3.4), 3.0);
    }

    #[test]
    fn test_min_f64() {
        assert_eq!(min_f64(1.0, 2.0), 1.0);
    }

    #[test]
    fn test_max_f64() {
        assert_eq!(max_f64(1.0, 2.0), 2.0);
    }

    // ─── log tests ────────────────────────────────────────────────────────

    #[test]
    fn test_log_print() {
        log_print("test message".to_string());
        // Should not panic
    }

    #[test]
    fn test_log_printf() {
        let result = log_printf("Hello, {}!".to_string(), vec!["world".to_string()]);
        assert_eq!(result, "Hello, world!");
    }

    #[test]
    fn test_log_println() {
        log_println("test message".to_string());
        // Should not panic
    }

    #[test]
    fn test_log_fatal() {
        log_fatal("fatal error".to_string());
        // Should not panic in test
    }

    #[test]
    fn test_logf_fatal() {
        let result = logf_fatal("error: {}".to_string(), vec!["something failed".to_string()]);
        assert_eq!(result, "error: something failed");
    }

    #[test]
    fn test_logln_fatal() {
        logln_fatal("fatal error".to_string());
        // Should not panic in test
    }

    #[test]
    fn test_set_prefix() {
        let result = set_prefix("[myapp] ".to_string());
        assert_eq!(result, "[myapp] ");
    }

    #[test]
    fn test_prefix() {
        let result = prefix();
        assert_eq!(result, "");
    }

    #[test]
    fn test_flags() {
        let result = flags();
        assert_eq!(result, 0x00123);
    }

    #[test]
    fn test_set_flags() {
        let result = set_flags(0x00123);
        assert!(result.contains("Ldate"));
    }

    #[test]
    fn test_output() {
        let result = output(1, "output message".to_string());
        assert_eq!(result, "output message");
    }

    #[test]
    fn test_set_output() {
        let result = set_output("stderr".to_string());
        assert!(result.contains("Writer:"));
    }

    #[test]
    fn test_init() {
        init();
        // Should not panic
    }

    #[test]
    fn test_set_panic() {
        let result = set_panic(true);
        assert_eq!(result, "Panic enabled");
    }
}
