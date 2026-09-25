pub mod context;
pub mod error;
pub mod message;
pub mod tokio;
pub mod utils;
pub mod image;
pub mod resources;

pub mod m;



pub struct Wry<T:'static>{
    context:Context<T>
}





fn main() {
    println!("Hello, world!");
}
