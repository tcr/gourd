use gourd_macro::go;
use gourd::prelude::{Complex64, Complex128};

// Test: complex() returns Complex128 (Go semantics)
go! {
    func goComplexBasic() Complex128 {
        c := complex(1.0, 2.0)
        return c
    }
}

// Test: real() and imag() extraction
go! {
    func goComplexExtractParts() float64 {
        c := complex(3.0, 4.0)
        r := real(c)
        i := imag(c)
        return r + i
    }
}

// Test: complex arithmetic (addition via overloaded operators)
go! {
    func goComplexArithmetic() float64 {
        a := complex(1.0, 2.0)
        b := complex(3.0, 4.0)
        c := a + b
        return real(c) + imag(c)
    }
}

#[test]
fn test_complex_basic() {
    let c = goComplexBasic();
    assert!((c.real - 1.0).abs() < 1e-6);
    assert!((c.imag - 2.0).abs() < 1e-6);
}

#[test]
fn test_complex_extract_parts() {
    let result = goComplexExtractParts();
    assert!((result - 7.0).abs() < 1e-12); // real=3, imag=4, 3+4=7
}

#[test]
fn test_complex_arithmetic() {
    let result = goComplexArithmetic();
    assert!((result - 10.0).abs() < 1e-6); // (1+2i)+(3+4i) = 4+6i, real+imag = 10
}
