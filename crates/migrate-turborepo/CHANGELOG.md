## 0.1.0

#### 🚀 Updates

- Removed the requirement of moon's project graph. Will now scan for `package.json`s instead.
- Cleaned up the migration code to be more readable and maintainable.

## 0.0.2

#### 🚀 Updates

- Updated to allow a missing or empty `pipeline` in `turbo.json`.

## 0.0.1

#### 🚀 Updates

- Initial release!
- New features from moon migration:
  - Bun support behind a new `--bun` flag.
  - Runs scripts through a package manager, instead of `moon node run-script`.
  - Root-level tasks will now create a root config, instead of warning.
  - Supports `globalDotEnv`, `dotEnv`, and `outputMode`.
