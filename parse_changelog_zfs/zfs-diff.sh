zfs diff -F $1 $2 | awk '{ if ( $2 == "/" ) { print $3 } else { system("dirname \"" $3" \"") } }' 
