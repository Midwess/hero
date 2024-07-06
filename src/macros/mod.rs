#[macro_export]
macro_rules! btreemap {
    ( $b:expr; $($x:expr => $y:expr),* ) => ({
        let mut temp_map = BTreeMap::with_b($b);
        $(
            temp_map.insert($x, $y);
        )*
        temp_map
    });
    ( $($x:expr => $y:expr),* ) => ({
        let mut temp_map = BTreeMap::new();
        $(
            temp_map.insert($x, $y);
        )*
        temp_map
    });
    ( $b:expr; $($x:expr => $y:expr,)* ) => (
        btreemap!{$b; $($x => $y),*}
    );
    ( $($x:expr => $y:expr,)* ) => (
        btreemap!{$($x => $y),*}
    );
}
