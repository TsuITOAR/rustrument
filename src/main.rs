#![allow(dead_code)]
use std::{
    error::Error,
    io::{BufRead, Read, Write},
    net::{IpAddr, SocketAddr, TcpStream, ToSocketAddrs, UdpSocket},
    time::Duration,
};

use rustrument::{
    instruments::{infiniium::Infiniium, Messenger},
    protocols::{
        onc_rpc::{
            vxi11::{DeviceFlags, Vxi11Client},
            RpcProgram,
        },
        Protocol, Tcp,
    },
    scpi::Scpi,
    DefaultConfig, PiezoController,
};
fn get_local_ip() -> Option<IpAddr> {
    let socket = match UdpSocket::bind("0.0.0.0:0") {
        Ok(s) => s,
        Err(_) => return None,
    };

    match socket.connect("8.8.8.8:80") {
        Ok(()) => (),
        Err(_) => return None,
    };

    match socket.local_addr() {
        Ok(addr) => return Some(addr.ip()),
        Err(_) => return None,
    };
}

const TIME_OUT: std::time::Duration = Duration::from_secs(1);
fn main() -> Result<(), Box<dyn Error>> {
    test_vxi11_connect("192.168.3.96".parse()?)?;
    Ok(())
}
fn scpi_test(addr: IpAddr) -> Result<(), Box<dyn Error>> {
    let mut client = Vxi11Client::default();
    client.lock = false;
    client.flags = DeviceFlags::new_zero();
    let mut connect = client.connect(addr, TIME_OUT)?;
    println!("set mask to 255");
    connect.set_event_mask(255)?;
    connect.set_service_mask(255)?;
    println!("after connecting");
    println!("{:?}", connect.get_status_byte()?);
    println!("{:?}", connect.get_event_byte()?);
    connect.scpi_send("*IDN?")?;
    println!("after sending valid command without reading");
    println!("{:?}", connect.get_status_byte()?);
    println!("{:?}", connect.get_event_byte()?);
    println!("{}", connect.scpi_query("*IDN?")?);
    println!("after sending valid command and reading");
    println!("{:?}", connect.get_status_byte()?);
    println!("{:?}", connect.get_event_byte()?);
    connect.scpi_send("s")?;
    println!("after send invalid command");
    println!("{:?}", connect.get_status_byte()?);
    println!("{:?}", connect.get_event_byte()?);
    connect.scpi_send(":SYSTem:DSP \"Hello World\"")?;
    println!("after display a message");
    println!("{:?}", connect.get_status_byte()?);
    println!("{:?}", connect.get_event_byte()?);
    Ok(())
}
fn test_vxi11_connect(addr: IpAddr) -> Result<(), Box<dyn Error>> {
    let mut client_osc = Vxi11Client::default();
    client_osc.lock = false;
    client_osc.flags = DeviceFlags::new_zero();
    let mut osc = client_osc.connect(addr, TIME_OUT)?;
    if let Err(e) = osc.establish_interrupt((get_local_ip().unwrap(), 3480), 3457, 1) {
        println!("establish interrupt channel failed: {}", e);
    };

    osc.device_write_str("*IDN?")?;
    println!("{:#010b}", osc.device_read_stb()?);
    println!("{}", osc.device_read_str()?);
    println!("{:#010b}", osc.device_read_stb()?);
    osc.device_enable_srq(true, "srq\n")?;

    Ok(())
}
fn test_port_mapper<B: ToSocketAddrs + Clone>(remote_addr: B) -> Result<(), Box<dyn Error>> {
    use rustrument::protocols::onc_rpc::port_mapper::*;
    let remote_addr = remote_addr.to_socket_addrs()?.next().unwrap();
    let dur = Duration::from_secs(1);
    let local_ip = get_local_ip().ok_or("error getting local ip address")?;
    let port = 12902;
    let local_addr = SocketAddr::new(local_ip, port);
    //tcp test
    let prog = <PortMapper<TcpStream> as RpcProgram>::PROGRAM;
    let vers = <PortMapper<TcpStream> as RpcProgram>::VERSION;
    {
        let mut tcp_handler = PortMapper::new_tcp(remote_addr.clone(), dur)?;
        println!("{}", tcp_handler.tcp_port(prog, vers)?);
        println!("{}", tcp_handler.udp_port(prog, vers)?);
    }

    {
        let mut udp_handler = PortMapper::new_udp(local_addr, remote_addr, dur)?;
        println!("{}", udp_handler.tcp_port(prog, vers)?);
        println!("{}", udp_handler.udp_port(prog, vers)?);
    }

    {
        let mut broad_caster = PortMapper::new_broadcaster(local_addr, dur)?;
        {
            let mut port_stream = broad_caster.collet_tcp_port(prog, vers, "224.0.0.1:111")?;
            loop {
                match port_stream.next() {
                    Some(s) => match s {
                        Ok((p, a)) => println!("got reply {:0>5} from {}", p, a.to_string()),
                        Err(e) => {
                            if e.is_timeout() {
                                break;
                            } else {
                                return Err(e.into());
                            }
                        }
                    },
                    None => unreachable!(),
                }
            }
        }
        {
            let mut port_stream = broad_caster.collet_udp_port(prog, vers, "224.0.0.1:111")?;
            loop {
                match port_stream.next() {
                    Some(s) => match s {
                        Ok((p, a)) => println!("got reply {:0>5} from {}", p, a.to_string()),
                        Err(e) => {
                            if e.is_timeout() {
                                break;
                            } else {
                                return Err(e.into());
                            }
                        }
                    },
                    None => unreachable!(),
                }
            }
        }
    }
    Ok(())
}
fn test_piezo() -> Result<(), Box<dyn Error>> {
    println!("Starting PiezoController connecting test\n");

    let mut controller = PiezoController::new(5)?;
    controller.set_x(30.)?;
    controller.set_y(30.)?;
    controller.set_z(30.)?;
    controller.update()?;
    println!("{}", controller);
    Ok(())
}

fn test_osc() -> Result<(), Box<dyn Error>> {
    print!("Starting Oscilloscope connecting test\n");
    let mut osc = Infiniium::default_connect("169.254.209.174:5025".parse()?)?;
    osc.send_raw("*IDN?")?;
    println!("{}", String::from_utf8_lossy(osc.read_until(b'\n')?));
    osc.send_raw(":WAVeform:SOURce CHANnel1")?;
    osc.send_raw(":WAVeform:DATA?")?;
    println!(
        "{:?}",
        std::str::from_utf8(osc.read_until(b'\n')?)?
            .lines()
            .next()
            .expect("no data received")
            .split_terminator(',')
            .map(|x| -> f32 { x.parse().unwrap() })
            .collect::<Vec<f32>>()
    );
    osc.send_raw(":ACQuire:SRATe:ANALog 250E+6")?;
    osc.send_raw("STOP")?;
    Ok(())
}

fn test_osc_rigol() -> Result<(), Box<dyn Error>> {
    let m = Tcp::default();
    let mut mess = std::io::BufReader::new(Messenger::new(
        m.connect("169.254.120.131:5555".parse()?, TIME_OUT)?,
    ));
    println!("connect success");
    mess.get_mut().write(":WAVeform:FORMat ASCii\n".as_ref())?;
    mess.get_mut().write(":WAVeform:MODE MAXimum\n".as_ref())?;
    for i in [1].iter() {
        mess.get_mut()
            .write(format!(":WAVeform:STOP {}\n", i * 1000_000).as_ref())?;
        mess.get_mut().write(":WAVeform:DATA?\n".as_ref())?;
        let mut buf = Vec::new();
        mess.read_until(b'\n', &mut buf)?;
        println!("read success");
        println!(
            "{}\n{}",
            String::from_utf8_lossy(buf.as_ref()),
            buf.len() / 14,
        );
    }

    Ok(())
}

fn test_awg_rigol() -> Result<(), Box<dyn Error>> {
    let m = Tcp;
    let mut mess = std::io::BufReader::new(Messenger::new(
        m.connect("192.168.3.94:111".parse()?, TIME_OUT)?,
    ));
    println!("connect success");

    let mut buf = [0_u8; 4];
    mess.read(&mut buf)?;
    println!("{}", u32::from_be_bytes(buf));
    Ok(())
}
