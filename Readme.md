# Simple Java Version Manager

## Motivation

A few years ago, I created a project to manage the Java version currently used on my machine and within a specific terminal prompt: [blazingly-fast-java-version-manager](https://github.com/theCat69/blazingly-fast-java-version-manager).
At the time, I was just starting to learn Rust and wanted to try a different approach than the classic symlink indirection everyone uses. It worked for most use cases, but the code was messy and overly complicated. I didn’t improve it much after that.

However, I kept using that tool over the years and wanted to build something better for myself.

This project aims to be a minimalist, simple, and cross-platform Java version manager using symlink indirection.

## Prerequisites and Permissions

To use `sjvm`, you must have permission to read, execute, and create symlinks for the JDK folders you want to manage.

### Windows

On Windows, you need to have Developer Mode enabled: [enable-your-device-for-development](https://learn.microsoft.com/fr-fr/windows/apps/get-started/enable-your-device-for-development)

Default JDK folder:

```batch
C:\Java
```

### Linux

On Linux, if you install JDKs via a package manager and your user cannot create symlinks in those locations, you’ll need to copy the JDKs to a folder you own.

Default JDK folder:

```sh
/usr/lib/jvm
```

### macOS

This is not tested. If you try it and it doesn't work, feel free to open an issue.

Default JDK folder:

```sh
/Library/Java/JavaVirtualMachines
```

## Configuration

`sjvm` uses JSON for its configuration.
A simple example:

```json
{
  "jdks_dirs": [
    "C:\\dev\\compilers\\java"
  ]
}
```

You can also specify the folder `sjvm` will use as the main symlink destination:

```json
{
  "symlink_dir": "C:\\dev\\sjvm\\java"
}
```

The configuration folder depends on your system. To find the path `sjvm` uses, run:

```sh
sjvm config path
```

## Setup

To load all your JDKs into memory and create the symlink folder, run:

```sh
sjvm setup
```

## Commands

### List

List Java installations managed by `sjvm` on your device:

```sh
sjvm list
```

Example output:

```
C:\dev\compilers\Java\jdk-17.0.1  
C:\dev\compilers\Java\jdk-20.0.1  
C:\dev\compilers\Java\jdk-21.0.1
```

### Use

To change the Java installation for your user:

```sh
sjvm use jdk-21
```

Example output:

```
✅ Now using JDK: C:\dev\compilers\Java\jdk-21.0.1
```

`sjvm` will match the name of the folder resolved by the list command.
It will use the first match, so name you folders accordingly.
