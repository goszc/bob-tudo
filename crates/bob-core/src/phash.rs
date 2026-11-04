use image::{imageops::FilterType, DynamicImage};

/// dHash de 64 bits: 9×8, cada bit compara o pixel com o vizinho da direita.
pub fn dhash(img: &DynamicImage) -> u64 {
    let small = img.resize_exact(9, 8, FilterType::Triangle).to_luma8();
    let mut bits = 0u64;
    let mut index = 0;
    for y in 0..8 {
        for x in 0..8 {
            let left = small.get_pixel(x, y)[0];
            let right = small.get_pixel(x + 1, y)[0];
            if left > right {
                bits |= 1 << index;
            }
            index += 1;
        }
    }
    bits
}

pub fn hamming(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgb};

    fn solid(w: u32, h: u32, color: [u8; 3]) -> DynamicImage {
        DynamicImage::ImageRgb8(ImageBuffer::from_fn(w, h, |_, _| Rgb(color)))
    }

    #[test]
    fn identical_images_match() {
        let img = solid(48, 32, [20, 80, 200]);
        assert_eq!(hamming(dhash(&img), dhash(&img)), 0);
    }

    #[test]
    fn different_images_diverge() {
        let a = DynamicImage::ImageRgb8(ImageBuffer::from_fn(64, 40, |x, _| {
            Rgb([x as u8 * 4, 10, 10])
        }));
        let b = DynamicImage::ImageRgb8(ImageBuffer::from_fn(64, 40, |x, _| {
            Rgb([10, 10, 255 - x as u8 * 3])
        }));
        assert!(hamming(dhash(&a), dhash(&b)) > 8);
    }
}
