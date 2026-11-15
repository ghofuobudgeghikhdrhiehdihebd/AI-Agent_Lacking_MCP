use std::time::Duration;

pub fn init_airlock() -> Result<Box<dyn serialport::SerialPort>, Box<dyn std::error::Error>> {
    let airlock = serialport::new("/dev/ttyGS0", 9600)
        .timeout(Duration::from_millis(100))
        .open()?;
    Ok(airlock)
}

pub fn read_line(
    airlock: &mut dyn serialport::SerialPort,
) -> Result<String, Box<dyn std::error::Error>> {
    Ok(rmp_serde::from_read(airlock)?)
}

pub fn send_str(
    airlock: &mut dyn serialport::SerialPort,
    data: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    airlock.write_all(rmp_serde::to_vec(data).unwrap().as_slice())?;
    airlock.flush()?;
    Ok(())
}