use std::net::{IpAddr, Ipv4Addr};

/// 检测是否为内网 IP 地址
pub fn is_private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => is_private_ipv4(ipv4),
        // 如果是 IPv6，可以在这里添加相应的检测逻辑
        IpAddr::V6(_) => false, // 暂时只处理 IPv4
    }
}

/// 检测 IPv4 地址是否为内网地址
pub fn is_private_ipv4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    
    // 10.0.0.0/8
    if octets[0] == 10 {
        return true;
    }
    
    // 172.16.0.0/12
    if octets[0] == 172 && (16..=31).contains(&octets[1]) {
        return true;
    }
    
    // 192.168.0.0/16
    if octets[0] == 192 && octets[1] == 168 {
        return true;
    }
    
    // 127.0.0.0/8 (环回地址)
    if octets[0] == 127 {
        return true;
    }
    
    // 169.254.0.0/16 (链路本地)
    if octets[0] == 169 && octets[1] == 254 {
        return true;
    }
    
    false
}

/// 获取本机内网 IP 地址（辅助函数）
pub fn get_local_private_ip() -> Option<Ipv4Addr> {
    if let Ok(socket) = std::net::UdpSocket::bind("0.0.0.0:0") {
        if let Ok(()) = socket.connect("8.8.8.8:80") {
            if let Ok(addr) = socket.local_addr() {
                if let IpAddr::V4(ipv4) = addr.ip() {
                    if is_private_ipv4(ipv4) {
                        return Some(ipv4);
                    }
                }
            }
        }
    }
    None
}