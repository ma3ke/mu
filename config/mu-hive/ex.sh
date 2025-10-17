#!/bin/bash
set -euxo pipefail

base=/martini/marieke/mu-experiment

machines=$base/machines.ini
output=$base/mu.dat
bee=$base/mu-bee
beelog=$base/beelog
log=$base/hive.log

# If it exists, back up the old log file.
[ ! -f $log ] || mv $log $log.old
$base/mu-hive --machines $machines --output $output --bee $bee --bee-log $beelog 2> $log
