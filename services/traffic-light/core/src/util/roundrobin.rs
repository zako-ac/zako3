pub fn next<'a, T>(arr: &'a [T], current: u16) -> (&'a T, u16) {
    let next_index = (current as usize + 1) % arr.len();
    (&arr[next_index], next_index as u16)
}
