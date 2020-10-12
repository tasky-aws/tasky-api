use std::io::{self,BufRead};

fn main() {
    let mut line = String::new();
    let stdin = io::stdin();
    stdin.lock().read_line(&mut line).unwrap();

    let mut line = String::new();
    stdin.lock().read_line(&mut line).unwrap();

    let args: Vec<&str> = line.split(" ").collect();
    let mut args: Vec<&str> = args.iter().map(|arg| arg.trim()).collect();

    args.sort();
    println!("{}", args.join(" "));

}