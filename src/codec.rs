use serde::de::DeserializeOwned;
use serde::Serialize;
use std::io::{Read, Write};

/// Serialize `message` with `bincode` and write length-prefixed bytes to `writer`.
pub fn send_message<W: Write, T: Serialize>(writer: &mut W, message: &T) -> std::io::Result<()> {
    let bytes = bincode::serialize(message)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err.to_string()))?;
    let len: u32 = bytes
        .len()
        .try_into()
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "message too large"))?;

    writer.write_all(&len.to_be_bytes())?;
    writer.write_all(&bytes)?;
    writer.flush()?;
    Ok(())
}

/// Read a length-prefixed message from `reader` and deserialize with `bincode`.
pub fn recv_message<R: Read, T: DeserializeOwned>(reader: &mut R) -> std::io::Result<Option<T>> {
    let mut len_buf = [0_u8; 4];

    match reader.read_exact(&mut len_buf) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(err) => return Err(err),
    }

    let len = u32::from_be_bytes(len_buf) as usize;
    let mut payload = vec![0_u8; len];
    reader.read_exact(&mut payload)?;

    let value = bincode::deserialize::<T>(&payload)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err.to_string()))?;
    Ok(Some(value))
}
