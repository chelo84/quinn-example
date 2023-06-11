use async_trait::async_trait;
use quinn::{Chunk, RecvStream, SendStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

#[async_trait]
pub trait Payload {
    async fn read_from_recv_stream(recv: &mut RecvStream) -> anyhow::Result<Self>
    where
        Self: Sized;

    async fn write_to_send_stream(&self, send: &mut SendStream) -> anyhow::Result<()>;
}

#[async_trait]
impl Payload for u8 {
    async fn read_from_recv_stream(recv: &mut RecvStream) -> anyhow::Result<u8> {
        Ok(recv.read_u8().await?)
    }

    async fn write_to_send_stream(&self, send: &mut SendStream) -> anyhow::Result<()> {
        send.write_u8(*self).await?;

        Ok(())
    }
}

#[async_trait]
impl Payload for u16 {
    async fn read_from_recv_stream(recv: &mut RecvStream) -> anyhow::Result<u16> {
        Ok(recv.read_u16().await?)
    }

    async fn write_to_send_stream(&self, send: &mut SendStream) -> anyhow::Result<()> {
        send.write_u16(*self).await?;

        Ok(())
    }
}

#[async_trait]
impl Payload for u32 {
    async fn read_from_recv_stream(recv: &mut RecvStream) -> anyhow::Result<u32> {
        Ok(recv.read_u32().await?)
    }

    async fn write_to_send_stream(&self, send: &mut SendStream) -> anyhow::Result<()> {
        send.write_u32(*self).await?;

        Ok(())
    }
}

#[async_trait]
impl Payload for u64 {
    async fn read_from_recv_stream(recv: &mut RecvStream) -> anyhow::Result<u64> {
        Ok(recv.read_u64().await?)
    }

    async fn write_to_send_stream(&self, send: &mut SendStream) -> anyhow::Result<()> {
        send.write_u64(*self).await?;

        Ok(())
    }
}

#[async_trait]
impl Payload for i8 {
    async fn read_from_recv_stream(recv: &mut RecvStream) -> anyhow::Result<i8> {
        Ok(recv.read_i8().await?)
    }

    async fn write_to_send_stream(&self, send: &mut SendStream) -> anyhow::Result<()> {
        send.write_i8(*self).await?;

        Ok(())
    }
}

#[async_trait]
impl Payload for i16 {
    async fn read_from_recv_stream(recv: &mut RecvStream) -> anyhow::Result<i16> {
        Ok(recv.read_i16().await?)
    }

    async fn write_to_send_stream(&self, send: &mut SendStream) -> anyhow::Result<()> {
        send.write_i16(*self).await?;

        Ok(())
    }
}

#[async_trait]
impl Payload for i32 {
    async fn read_from_recv_stream(recv: &mut RecvStream) -> anyhow::Result<i32> {
        Ok(recv.read_i32().await?)
    }

    async fn write_to_send_stream(&self, send: &mut SendStream) -> anyhow::Result<()> {
        send.write_i32(*self).await?;

        Ok(())
    }
}

#[async_trait]
impl Payload for i64 {
    async fn read_from_recv_stream(recv: &mut RecvStream) -> anyhow::Result<i64> {
        Ok(recv.read_i64().await?)
    }

    async fn write_to_send_stream(&self, send: &mut SendStream) -> anyhow::Result<()> {
        send.write_i64(*self).await?;

        Ok(())
    }
}

#[async_trait]
impl Payload for f32 {
    async fn read_from_recv_stream(recv: &mut RecvStream) -> anyhow::Result<f32> {
        Ok(recv.read_f32().await?)
    }

    async fn write_to_send_stream(&self, send: &mut SendStream) -> anyhow::Result<()> {
        send.write_f32(*self).await?;

        Ok(())
    }
}

#[async_trait]
impl Payload for f64 {
    async fn read_from_recv_stream(recv: &mut RecvStream) -> anyhow::Result<f64> {
        Ok(recv.read_f64().await?)
    }

    async fn write_to_send_stream(&self, send: &mut SendStream) -> anyhow::Result<()> {
        send.write_f64(*self).await?;

        Ok(())
    }
}

#[async_trait]
impl Payload for String {
    async fn read_from_recv_stream(recv: &mut RecvStream) -> anyhow::Result<String> {
        let string_size: u16 = recv.read_u16().await?;

        let chunk: Chunk = recv.read_chunk(string_size.into(), true).await?.unwrap();
        let string: String = String::from_utf8(chunk.bytes.to_vec())?;

        Ok(string)
    }

    async fn write_to_send_stream(&self, send: &mut SendStream) -> anyhow::Result<()> {
        let bytes: &[u8] = self.as_bytes();
        let string_length = u16::try_from(bytes.len()).expect("Unable to parse usize to u16");

        // write the length of the string
        send.write_u16(string_length).await?;
        // write the string
        send.write(bytes).await?;

        Ok(())
    }
}

/// If the first byte is:
/// 0b1 => Some
/// _ => None
#[async_trait]
impl<T> Payload for Option<T>
where
    T: Sync + Payload,
{
    async fn read_from_recv_stream(recv: &mut RecvStream) -> anyhow::Result<Option<T>> {
        let first_byte = recv.read_u8().await?;
        let result: Option<T> = match first_byte {
            0b1 => Some(T::read_from_recv_stream(recv).await?),
            _ => None,
        };

        Ok(result)
    }

    async fn write_to_send_stream(&self, send: &mut SendStream) -> anyhow::Result<()> {
        match self {
            Some(x) => {
                send.write_u8(0b1).await?;

                x.write_to_send_stream(send).await?;
            }
            None => {
                send.write_u8(0b0).await?;
            }
        };

        Ok(())
    }
}

#[async_trait]
impl Payload for Uuid {
    async fn read_from_recv_stream(recv: &mut RecvStream) -> anyhow::Result<Uuid> {
        Ok(Uuid::from_u128(recv.read_u128().await?))
    }

    async fn write_to_send_stream(&self, send: &mut SendStream) -> anyhow::Result<()> {
        send.write_u128(self.as_u128()).await?;

        Ok(())
    }
}
