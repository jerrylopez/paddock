# common.sh: shared prelude for every install script.
#
# Paddock places this file in front of the service script and pipes both to
# bash as one script, so everything defined here is available to the service.
# Service scripts receive the version as $1 and any extra flags after it.

set -euo pipefail
