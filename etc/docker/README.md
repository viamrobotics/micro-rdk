## build 

note: the build final process may take a long time (>1hr)
```
$ docker run --rm --privileged multiarch/qemu-user-static --reset -p yes -c yes
$ make -f docker.make micro-rdk-dev
```

## upload
```
$ make -f docker.make micro-rdk-upload
```
