// A user error enum: `impl Error` is a no-op, Debug and Display both work.
#[derive(Debug)]
enum AppError {
    Bad,
    Worse,
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            AppError::Bad => write!(f, "bad"),
            AppError::Worse => write!(f, "worse"),
        }
    }
}

impl std::error::Error for AppError {}

fn fail(k: i32) -> Result<i32, AppError> {
    if k < 0 {
        Err(AppError::Bad)
    } else if k == 0 {
        Err(AppError::Worse)
    } else {
        Ok(k)
    }
}

fn main() {
    println!("{:?} {:?}", fail(2), fail(-1));
    let e = fail(-1).unwrap_err();
    println!("{} {:?}", e, e);
    println!("{:?}", fail(0));
}
