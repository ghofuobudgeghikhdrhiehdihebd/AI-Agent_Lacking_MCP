use agent_no_mcp_rust::serial_utils::ArduinoSerial;

fn main() {
    let mut uart = ArduinoSerial::new().expect("Failed to initialize Arduino UART");
    uart.send_text("Test message")
        .expect("Failed to send text over UART");
    uart.send_msgpack(&"{'test': 'value'}")
        .expect("Failed to send MessagePack over UART");
}
