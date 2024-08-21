# The problem

We have to deploy an executable binary on a bunch of networked machines where
the input data are stored on a networked file system, visible from each of those
machines. So far, so good. But:

1. We have no admin rights on any of the machines.

2. The shared filesystem removes executable bits from any files placed there by
   a mere user.

3. There seem to be no problems with the` x` bits on the local filesystems.

# The hack

1. Provide a shell script that creates the binary and copies it to some location
   in the shared filesystem (so that normal users can execute it from any
   machine), along with ...

2. ... a second script which

   1. copies the binary from the shared location into a temporary space on the local filesystem,

   2. sets the executable bit on the local copy of the binary

   3. executes the binary, passing on any arguments it has received.

The point being

1. that the *script* can be executed with `sh` so doesn't need the `x` bits,

2. the *binary* must have the `x` bit set, but this can only be done in the local filesystem,

3. the script acts as a proxy for the binary.
