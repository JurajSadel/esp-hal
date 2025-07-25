use crate::aes::{ALIGN_SIZE, Aes, Endianness, Mode};

impl Aes<'_> {
    pub(super) fn init(&mut self) {
        self.write_endianness(
            Endianness::BigEndian,
            Endianness::BigEndian,
            Endianness::BigEndian,
            Endianness::BigEndian,
            Endianness::BigEndian,
            Endianness::BigEndian,
        );
    }

    pub(super) fn write_key(&mut self, key: &[u8]) {
        let key_len = self.regs().key_iter().count();
        debug_assert!(key.len() <= key_len * ALIGN_SIZE);
        debug_assert_eq!(key.len() % ALIGN_SIZE, 0);
        self.alignment_helper
            .volatile_write_regset(self.regs().key(0).as_ptr(), key, key_len);
    }

    pub(super) fn write_block(&mut self, block: &[u8]) {
        let text_len = self.regs().text_iter().count();
        debug_assert_eq!(block.len(), text_len * ALIGN_SIZE);
        self.alignment_helper
            .volatile_write_regset(self.regs().text(0).as_ptr(), block, text_len);
    }

    pub(super) fn write_mode(&self, mode: Mode) {
        self.regs().mode().write(|w| unsafe { w.bits(mode as _) });
    }

    /// Configures how the state matrix would be laid out
    pub fn write_endianness(
        &mut self,
        input_text_word_endianess: Endianness,
        input_text_byte_endianess: Endianness,
        output_text_word_endianess: Endianness,
        output_text_byte_endianess: Endianness,
        key_word_endianess: Endianness,
        key_byte_endianess: Endianness,
    ) {
        let mut to_write = 0_u32;
        to_write |= key_byte_endianess as u32;
        to_write |= (key_word_endianess as u32) << 1;
        to_write |= (input_text_byte_endianess as u32) << 2;
        to_write |= (input_text_word_endianess as u32) << 3;
        to_write |= (output_text_byte_endianess as u32) << 4;
        to_write |= (output_text_word_endianess as u32) << 5;
        self.regs().endian().write(|w| unsafe { w.bits(to_write) });
    }

    pub(super) fn write_start(&self) {
        self.regs().start().write(|w| w.start().set_bit());
    }

    pub(super) fn read_idle(&mut self) -> bool {
        self.regs().idle().read().idle().bit_is_set()
    }

    pub(super) fn read_block(&self, block: &mut [u8]) {
        let text_len = self.regs().text_iter().count();
        debug_assert_eq!(block.len(), text_len * ALIGN_SIZE);
        self.alignment_helper
            .volatile_read_regset(self.regs().text(0).as_ptr(), block, text_len);
    }
}
