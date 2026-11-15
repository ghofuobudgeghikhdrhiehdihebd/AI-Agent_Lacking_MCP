use agent_no_mcp_rust::uart_utils::ArduinoUart;

fn main() {
    let mut uart = ArduinoUart::new().expect("Failed to initialize Arduino UART");
    uart.send_text("Test message").expect("Failed to send text over UART");
    uart.send_msgpack(&"{'test': 'value'}").expect("Failed to send MessagePack over UART");
}