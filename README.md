[![Build Status](https://api.travis-ci.org/Yoric/sublock.svg?branch=master)](https://travis-ci.org/Yoric/sublock)

[Documentation](http://yoric.github.io/sublock/doc/sublock/)


Variants of `RwLock` that support sublocks, opened for reading if the main `RwLock`
is opened for reading, opened for writing if the main `RwLock` is opened for writing.

This crate has been designed to permit refactoring of code using `RefCell` into
`Sync` code.

