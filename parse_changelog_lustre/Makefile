LUSTRE_LIBRARY_DIR=/home/benja/lustre-release/lustre/utils/.libs
LUSTRE_INCLUDE_DIR=/home/benja/lustre-release/include

parse_changelog_lustre:
	gcc -g -D_GNU_SOURCE -I$(LUSTRE_INCLUDE_DIR) -I$(LUSTRE_INCLUDE_DIR)/uapi -L$(LUSTRE_LIBRARY_DIR) -llustreapi parse_changelog_lustre.c -o parse_changelog_lustre	

clean:
	rm -f parse_changelog_lustre
