# Simple Java Version Manager

## Motivations :

I did a project few years ago to handle java version currently used on my machine and in a particuliar prompt : https://github.com/theCat69/blazingly-fast-java-version-manager.
At the time i was just starting learning Rust and I wanted to try a different approach that the classic symlink indirection that everyone use. It worked, for most usecase but it was so complicated because my code was messy. I did not improved that much. However I wanted to do this for myself as I have been using my previous tool during those years. 

This project aims to be a minimalist and simple, multiplatform, java version manager using symlink indirection.

## Prerequist and system rights considerations

The user using this should have the right to read, execute and create symlinks of the jdks folders one wish to use with sjvm.

### Windows 

On windows you need to have developer mode on : https://learn.microsoft.com/fr-fr/windows/apps/get-started/enable-your-device-for-development.

Default folder for jdks is :
```batch
C:\Java
```

### Linux 

On linux, if you want tu use the package from your package manager, and the user you are using can't create symlinks on them, you will need to copy them to a folder you own.

Default folder for jdks is :
```sh
/usr/lib/jvm
```
### Mac 

This is not tested at all. If you try and it doesn't work you can open an issue.

Default folder for jdks is : 
```sh
/Library/Java/JavaVirtualMachines
```

## Configuration

Sjvm use Json configuration. 
A simple configuration can look like so : 
```json
{
  "jdks_dirs": [
    "C:\\dev\\compilers\\java"
  ]
}
```

You can also choose the folder sjvm will use as the main source folder of the symlink.
```json
{
  "symlink_dir": "C:\\dev\\sjvm\\java" 
}
```

Configuration folder depends on your system. To get the path sjvm will use you can run the command :
```sh
sjvm config path
```

## Setup
To setup sjvm to load all your jdks into his memory and create you symlink folder use :
```sh
sjvm setup
```

## Commands

### List
List java installations managed by sjvm on your device :
```sh
sjvm list
```

```
C:\dev\compilers\Java\jdk-17.0.1
C:\dev\compilers\Java\jdk-20.0.1
C:\dev\compilers\Java\jdk-21.0.1
```

### Use
To change java installation for your user run :
```sh
sjvm use jdk-21
```

```
✅ Now using JDK: C:\dev\compilers\Java\jdk-21.0.1
```

Sjvm will match the name of the folder you see in the list command.
It will use the first match so name you folders accordingly.




