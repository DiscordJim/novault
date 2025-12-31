novovault
------------
NovoVault is a personal note encryption application that allows you to store notes securely on the cloud. Right now, it is designed for GitHub usage but I would be pleased to add supposed for other platforms if an issue is opened.

[![Crates.io](https://img.shields.io/crates/v/novovault.svg)](https://crates.io/crates/novovault)

### Documentation Quick Links
* [Installation](#installation)
* [QuickStart](#quickstart)
* [Acknowledgements](#acknowledgements)


### Installation
The binary name for NovoVault is `novovault`. The tool is primarily designed to be installed via cargo:
```
$ cargo install novovault
```


### Quickstart
This part is designed to get you up and running as fast as possible. To begin, you need an empty directory and you run the following command:
```
$ novovault init
```
which will initialize a new repository and immediately put it in the sealed state. To unseal the vault, you can run,
```
$ novovault unseal
```
and to reseal,
```
$ novovault seal
```
I recommend running it in 'open' mode, which is where you run,
```
$ novovault open
```
where it will keep the vault open while you work on it and shut it down when you close the application.

To now link it to a remote repository, you can run link:
```
$ novovault link git@github.com:DiscordJim/novault.git
```


### Acknowledgements
I would like to thank Andrew Heschl for his contributions to this tool.