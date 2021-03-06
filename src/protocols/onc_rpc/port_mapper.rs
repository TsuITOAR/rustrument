use super::{xdr::*, IpProtocol, Result, Rpc, RpcBroadcast, RpcProgram, RpcSocket, RpcStream};
use bytes::{Buf, BytesMut};
use std::{
    net::{SocketAddr, TcpStream, ToSocketAddrs, UdpSocket},
    time::Duration,
};

pub const PORT: u16 = 111;

pub struct PortMapper<S> {
    io: S,
    buffer: BytesMut,
}

impl<S> PortMapper<S> {
    pub fn new(io: S) -> Self {
        Self {
            io,
            buffer: BytesMut::new(),
        }
    }
    pub fn get_io(&self) -> &S {
        &self.io
    }
    pub fn mut_io(&mut self) -> &mut S {
        &mut self.io
    }
}
impl<S> RpcProgram for PortMapper<S> {
    const PROGRAM: u32 = 100000;
    const VERSION: u32 = 2;
    type IO = S;
    fn buffer(&self) -> BytesMut {
        self.buffer.clone()
    }
    fn get_io(&self) -> &Self::IO {
        &self.io
    }
    fn mut_io(&mut self) -> &mut Self::IO {
        &mut self.io
    }
}
impl PortMapper<TcpStream> {
    pub fn new_tcp<D: Into<Option<Duration>> + Clone>(
        addr: SocketAddr,
        dur: D,
    ) -> Result<PortMapper<TcpStream>> {
        let io = TcpStream::connect_timeout(
            &addr,
            dur.clone().into().unwrap_or(Duration::from_secs(1)),
        )?;
        io.set_read_timeout(dur.into())?;
        Ok(PortMapper {
            io,
            buffer: BytesMut::new(),
        })
    }
}
impl PortMapper<UdpSocket> {
    pub fn new_udp<L: ToSocketAddrs, R: ToSocketAddrs, D: Into<Option<Duration>>>(
        local: L,
        remote: R,
        dur: D,
    ) -> Result<PortMapper<UdpSocket>> {
        let io = UdpSocket::bind(local)?;
        io.set_read_timeout(dur.into())?;
        io.connect(remote)?;
        Ok(PortMapper {
            io,
            buffer: BytesMut::new(),
        })
    }
    pub fn new_broadcaster<L: ToSocketAddrs, D: Into<Option<Duration>>>(
        local: L,
        dur: D,
    ) -> Result<PortMapper<UdpSocket>> {
        let io = UdpSocket::bind(local)?;
        io.set_read_timeout(dur.into())?;
        Ok(PortMapper {
            io,
            buffer: BytesMut::new(),
        })
    }
}
impl<S: RpcStream> PortMapper<S> {
    pub fn get_port(&mut self, prog: u32, vers: u32, ip_pro: IpProtocol) -> Result<u32> {
        let mut b: bytes::Bytes = self.call_anonymously(
            Procedure::GetPort,
            mapping {
                port: 0,
                prog,
                prot: match ip_pro {
                    IpProtocol::Tcp => IPPROTO_TCP,
                    IpProtocol::Udp => IPPROTO_UDP,
                },
                vers,
            },
        )?;
        Ok(b.get_u32())
    }
    pub fn tcp_port(&mut self, prog: u32, vers: u32) -> Result<u32> {
        self.get_port(prog, vers, IpProtocol::Tcp)
    }
    pub fn udp_port(&mut self, prog: u32, vers: u32) -> Result<u32> {
        self.get_port(prog, vers, IpProtocol::Udp)
    }
}

impl<S: RpcSocket> PortMapper<S> {
    pub fn collect_port<'a, A: ToSocketAddrs>(
        &'a mut self,
        prog: u32,
        vers: u32,
        ip_pro: IpProtocol,
        addr: A,
    ) -> Result<impl Iterator<Item = Result<(u32, SocketAddr)>> + 'a> {
        let stream = self.broadcast_anonymously(
            Procedure::GetPort,
            mapping {
                port: 0,
                prog,
                prot: match ip_pro {
                    IpProtocol::Tcp => IPPROTO_TCP,
                    IpProtocol::Udp => IPPROTO_UDP,
                },
                vers,
            },
            addr,
        )?;
        Ok(stream.map(
            |x: Result<(bytes::Bytes, SocketAddr)>| -> Result<(u32, SocketAddr)> {
                match x {
                    Ok((b, a)) => Ok((
                        serde_xdr::from_bytes::<_, u32>(b)
                            .map_err(|x| std::io::Error::new(std::io::ErrorKind::Other, x))?,
                        a,
                    )),
                    Err(e) => Err(e),
                }
            },
        ))
    }
    pub fn collet_tcp_port<'a, A: ToSocketAddrs + 'a>(
        &'a mut self,
        prog: u32,
        vers: u32,
        addr: A,
    ) -> Result<impl Iterator<Item = Result<(u32, SocketAddr)>> + 'a> {
        self.collect_port(prog, vers, IpProtocol::Tcp, addr)
    }
    pub fn collet_udp_port<'a, A: ToSocketAddrs + 'a>(
        &'a mut self,
        prog: u32,
        vers: u32,
        addr: A,
    ) -> Result<impl Iterator<Item = Result<(u32, SocketAddr)>> + 'a> {
        self.collect_port(prog, vers, IpProtocol::Udp, addr)
    }
}

pub enum Procedure {
    Set,
    Unset,
    GetPort,
    //pamamplist unsupported yet
    //Dump,
    CallIt,
}
impl Into<u32> for Procedure {
    fn into(self) -> u32 {
        use Procedure::*;
        match self {
            Set => 1,
            Unset => 2,
            GetPort => 3,
            //Dump=>4,
            CallIt => 5,
        }
    }
}
