fn main() {
    let client =
        attest_inference_router::from_env().expect("failed to build inference client from env");
    println!("attest-inference-router: provider={}", client.provider());
}
