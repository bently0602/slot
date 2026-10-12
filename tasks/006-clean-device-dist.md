# Clean the device distribution folder

Make `setup-device.sh` clean the generated `dist-device` folder so stale files
do not survive a successful rebuild. Keep the previous bundle if building or
validating the new bundle fails, and retain build caches.

Status: complete. Requested by the user.

The script cleans `dist-device` after validating its staging bundle, then copies
the fresh output. Help and README describe the cleanup. Smoke tests cover stale
software, hidden files, old content, reruns and preservation on build failure.

Validation: Bash syntax checks and `tests/test-setup-device.sh` passed in Ubuntu
through WSL.
