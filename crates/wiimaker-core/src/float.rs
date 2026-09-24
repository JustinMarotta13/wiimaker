//! f32 helpers that work with and without `std`.

#[inline]
pub fn sqrt(x: f32) -> f32 {
    #[cfg(feature = "std")]
    {
        x.sqrt()
    }
    #[cfg(not(feature = "std"))]
    {
        libm::sqrtf(x)
    }
}

#[inline]
pub fn floor(x: f32) -> f32 {
    #[cfg(feature = "std")]
    {
        x.floor()
    }
    #[cfg(not(feature = "std"))]
    {
        libm::floorf(x)
    }
}

#[inline]
pub fn powi2(x: f32) -> f32 {
    x * x
}
