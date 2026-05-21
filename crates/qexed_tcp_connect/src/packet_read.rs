use bytes::BytesMut;
use std::collections::VecDeque;
use tokio::io::AsyncRead;
use tokio_stream::Stream;
// 数据包读取
pub struct PacketStream<R> {
    reader: R,
    buffer: BytesMut,
    pending: VecDeque<bytes::BytesMut>,
}

impl<R: AsyncRead + Unpin> PacketStream<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            buffer: BytesMut::new(),
            pending: VecDeque::new(),
        }
    }
}

impl<R: AsyncRead + Unpin> Stream for PacketStream<R> {
    type Item = Result<bytes::BytesMut,PacketReadError>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        use std::pin::Pin;
        use tokio::io::ReadBuf;

        loop {
            if let Some(packet) = self.pending.pop_front() {
                return std::task::Poll::Ready(Some(Ok(packet)));
            }
            let packets = match self.try_parse_packets() {
                Ok(pkts) => pkts,
                Err(e) => return std::task::Poll::Ready(Some(Err(e))),
            };
            if !packets.is_empty() {
                self.pending.extend(packets);
                continue;
            }
            let mut tmp = [0u8; 1024];
            let mut buf = ReadBuf::new(&mut tmp);

            match Pin::new(&mut self.reader).poll_read(cx, &mut buf) {
                std::task::Poll::Pending => return std::task::Poll::Pending,
                std::task::Poll::Ready(Ok(())) => {
                    let n = buf.filled().len();
                    if n == 0 {
                        // EOF
                        if !self.buffer.is_empty() {
                            return std::task::Poll::Ready(Some(Err(PacketReadError::ConnectionClosedWithIncompletePacket)));
                        }
                        return std::task::Poll::Ready(None);
                    }
                    self.buffer.extend_from_slice(&tmp[..n]);
                }
                std::task::Poll::Ready(Err(e)) => {
                    return std::task::Poll::Ready(Some(Err(PacketReadError::OtherError(e))));
                }
            }
        }
    }
}

impl<R: AsyncRead + Unpin> PacketStream<R> {
    fn try_parse_packets(&mut self) -> Result<Vec<bytes::BytesMut>,PacketReadError> {
        let mut packets = Vec::new();
        let buffer = &mut self.buffer;
        while buffer.len() > 0 {
            let mut cursor = std::io::Cursor::new(buffer.as_ref());

            let len = match read_varint(&mut cursor) {
                Ok(v) => v,
                Err(e) => return Err(PacketReadError::PacketReadVarIntParseError(e)),
            };
            let header_len = cursor.position() as usize;
            let len = len as usize;
            let total = (header_len + len) as usize;

            if buffer.len() < total {
                break;
            }

            let mut payload = buffer.split_to(total);
            let payload = payload.split_off(header_len);

            packets.push(payload);
        }
        Ok(packets)
    }
}

#[derive(Debug, qexed_error_macros::I18nErrorDisplay)]
pub enum PacketReadError {
    #[error("qexed_tcp_connect.packet_read.packet_read_varint_error",error=field_0)]
    PacketReadVarIntParseError(PacketReadVarIntParseError),
    #[error("qexed_tcp_connect.packet_read.connection_closed_with_incomplete_packet")]
    ConnectionClosedWithIncompletePacket,
    #[error("qexed_tcp_connect.packet_read.io_error",error=field_0)]
    OtherError(std::io::Error)
}
impl std::error::Error for PacketReadError {}

fn read_varint<R: std::io::Read>(r: &mut R) -> Result<i32,PacketReadVarIntParseError> {
    let mut val = 0i32;
    let mut shift = 0;

    loop {
        let mut b = [0u8; 1];
        let n = match r.read(&mut b){
            Ok(v)=>v,
            Err(e)=>{
                return Err(PacketReadVarIntParseError::ReadError(e));
            }
        };
        if n == 0 {
            return Err(PacketReadVarIntParseError::IncompleteError);
        }

        val |= ((b[0] & 0x7F) as i32) << shift;
        if (b[0] & 0x80) == 0 {
            break;
        }

        shift += 7;
        if shift >= 35 {
            return Err(PacketReadVarIntParseError::TooLargeError);
        }
    }

    Ok(val)
}
#[derive(Debug, qexed_error_macros::I18nErrorDisplay)]
pub enum PacketReadVarIntParseError {
    #[error("qexed_tcp_connect.packet_read_varint.incomplete")]
    IncompleteError,
    #[error("qexed_tcp_connect.packet_read_varint.too_large")]
    TooLargeError,
    #[error("qexed_tcp_connect.packet_read_varint.read_error",error=field_0)]
    ReadError(std::io::Error)
}
impl std::error::Error for PacketReadVarIntParseError {}