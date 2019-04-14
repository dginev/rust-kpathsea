# Change Log

## [0.2.2] (next target)

## [0.2.1] 2019-04-14

 * Fix bug in `guess_format_from_filename`

## [0.2.0] 2019-04-11

* More robust detection of the tex toolchain, as expected by the `kpathsea` C library
   * making the wrapper more reliable to build and use cross-platform.
   * Thanks @xymostech for tracking down and upgrading.

## [0.1.3] 2019-03-12

### Added

* `find_file` can now discover the full range of `kpathsea`-supported types, via `guess_format_from_filename`. Thank you @xymostech !