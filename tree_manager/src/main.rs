use std::io::{self, BufRead};

fn main() {
    
    println!("Starting tree_manager");

    let stdin = io::stdin();
    let reader = stdin.lock();


    // input lines should be formatted like path\x00ino
    for line in reader.lines() {
        // println!("tree_manager: {}", line.unwrap());

        

    }

    println!("Finished tree_manager");
}
