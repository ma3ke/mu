# _mu_&mdash;show lab cluster usage

View and gather machine usage for our lab cluster.

A lot of details of this project are informed strictly by the architecture of
our lab cluster and the needs of the researchers. This project is therefore
less of a general-purpose system than a publicly available listing of what
works for us.

# Programs

Three related programs constitute our usage inforamtion system.

- `mu` is the terminal viewer for the user information.
    - This is only user-facing application and is simply a reader for the
      information.
    - The usage information is read from a periodically updated file typically
      called `mu.dat`.
- `mu-hive` is periodically executed to gather the usage information from a
  list of machines.
    - The list of machines is typically called `machines.ini`.
    - It establishes an ssh connection to the requested machines and from that
      connection executes a small executable called `mu-bee` which sends a
      serialized data stream of usage information for that machine over stdout.
    - The incoming information from multiple machines is integrated and written
      to the central `mu.dat` file that is read by `mu`.
- `mu-bee` gathers system information.
    - Information such as load averages, global cpu and memory figures, and
      some per-process information for significantly active processes are
      serialized and sent back to `mu-hive` over stdout.
    - The process names and users that are to be ignored or renamed before
      serializing are outlined in a configuration file called `ignore.linus`.

## `mu` viewer

The program `mu` just happens to be a terminal viewer for the information
gathered by `mu-hive` and `mu-bee`. This information may also be rendered for
the user by different means or with a different approach. For example, it may
be cool to play around with creating a web-view of the same information in
`mu.dat`.

## Execution of `mu-hive`

Some details regarding the execution of `mu-hive` may be important to know.
In our lab, `mu-hive` is executed from a specialized user account which has
permissions to ssh into all listed lab machines and has write permissions for
its own directory in the file server.
Through a cron job, `mu-hive` is periodically executed by running a small
script (`ex.sh`) every minute. This script backs up the `hive.log` file and
executes `mu-hive` with the correct arguments.

# Future work

- Keep track of more information such as
  - GPU memory and activity,
  - available memory (perhaps as a small visual gauge),
  - available storage over the different file systems we have mounted (would
    provide an early and obvious warning system to users).
- System for marking one's own machine as soft-reserved. This reservation is
  more of an indication not to run jobs on that system. Just an aid for
  communication. Perhaps the reservation should automatically time out after 24
  hours or early in the morning every day.
- Maybe we can hook in the online calendar for reserving `herman` and `alan`
  time into `mu`, as a way of displaying their reservations.
- Create a longitudinal graph of usage, so there is a couple hours worth of
  usage stored to a file as a coarse time series.
- Add tabs to show a visual representation of the rooms layout and what
  machines are where. Perhaps even what users are expected to use what machine
  until when. This would fold the administrative information that Linus has
  been keeping track of into a machine-readable format which can be rendered to
  the terminal or web interface for easy access.
- I think it would be very enjoyable to create a small webpage that renders the
  usage information in the browser. A nice and minimal html+some css look
  sounds very appealing to me.

# A historical note

This project is a new take on a system that has in place in the lab for many
years already. The original `machine_usage.py` has been performing the same
task in roughly the same way for what is probably more than a decade now. I
think that this program was written by Helgi, but I'd have to verify that to be
certain. For a long time, the usage information was rendered by a web page.

A couple of years ago, Jan created a command-line program called
`machine_usage` that rendered the `machine_usage.dat` information in the
terminal. `mu` is an outright copy of that excellent idea.

---

Marieke Westendorp, Jan Stevens, Linus Grünewald, 2025
