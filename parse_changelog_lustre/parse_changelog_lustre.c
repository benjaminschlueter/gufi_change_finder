// SPDX-License-Identifier: MIT
// Copyright 2025. Triad National Security, LLC.

#include <stdio.h>
#include <stdlib.h>
#include <getopt.h>
#include <libgen.h>
#include <limits.h>
#include <string.h>

#include "lustre/lustreapi.h"

static struct {
	char *consumer_name;	/* e.g., 'cl1' */
	char *mdt_name;		/* e.g., 'lustre-MDT0000' */
	char *filename;		/* file to store records in */
	bool filename_set;	/* true if a user-specified filename is used */
	uint64_t max_records;	/* the maximum number of records to process */
	bool keep_records;	/* whether to clear the records that were consumed */
} args = { 0 };

int parse_args(int argc, char *argv[])
{
	int opt;

	args.filename = "/dev/null";
	args.max_records = UINT64_MAX;

	static struct option long_options[] = {
		{"file", required_argument, 0, 'f'},
		{"consumer_id", required_argument, 0, 'i'},
		{"mdt",	  required_argument, 0, 'm'},
		{"keep", no_argument, 0, 1},
		{"max_records",	  required_argument, 0, 2},
		{"help",	 no_argument,	   0, 'h'},
		{0, 0, 0, 0}
	};

	while ((opt = getopt_long(argc, argv, "f:hi:m:", long_options, NULL)) != -1) {
		switch (opt) {
			case 'f':
				args.filename = optarg;
				args.filename_set = true;
				break;
			case 'i':
				args.consumer_name = optarg;
				break;
			case 'm':
				args.mdt_name = optarg;
				break;
			case 1:
				args.keep_records = true;
				break;
			case 2:
				if (sscanf(optarg, "%llu", &args.max_records) != 1) {
					fprintf(stderr, "Could not parse '%s'.\n", optarg);
					goto usage;
				}
				break;
			case 'h':
				goto usage;
			default:
				goto usage;
		}
	}

	if (!args.mdt_name || !args.consumer_name) {
		goto usage;
	}

	return 0;

usage:
	printf("Usage: %s --mdt MDT --consumer_id CONSUMER [--file LOG_FILE] [--max_records N] [--keep]\n", argv[0]);
	return 1;
}

/*
 * Process changelog records.
 *
 * After a certain number of records have been consumed, write out the log entries
 * to the log file and sync the file. Once this has been done, those records can be
 * cleared.
 *
 * Returns 0 for success, or -1 if any errors were observed.
 */
int process_records(void *changelog_priv, int mnt_fd, int log_fd, char* mnt_path)
{
	struct changelog_rec *rec;
	int num_records = 0;
	__u64 index = 0;
	int rc = 0;
	int p = 0;

	while ((rc = llapi_changelog_recv(changelog_priv, &rec)) == 0) {
		if (num_records++ >= args.max_records)
			break;

		index = rec->cr_index;

		__u32 type = rec->cr_type;

		/* Filter out record types that we aren't interested in: */
		if (type != CL_CREATE &&
			type != CL_MKDIR &&
			type != CL_HARDLINK &&
			type != CL_SOFTLINK &&
			type != CL_MKNOD &&
			type != CL_UNLINK &&
			type != CL_RMDIR &&
			type != CL_RENAME &&
			type != CL_SETATTR &&
			type != CL_SETXATTR &&
			type != CL_MTIME ) {
			continue;
		}

		long long recno;
		int linkno = 0;
		int linktmp = 0;	
		
		// print all paths associated with fid
		while (1) {
			
			char path_buf[PATH_MAX];
			char parent_buf[PATH_MAX]; 
			
			// if pfid is uninitialized, use dirname to get parent path
			if (rec->cr_pfid.f_seq == 0 && rec->cr_pfid.f_oid == 0 && rec->cr_pfid.f_ver == 0) {
				if (llapi_fid2path_at(mnt_fd, &rec->cr_tfid, path_buf, PATH_MAX, &recno, &linkno)) {
					perror("llapi_fid2path_at");
					return -1;
				}
					
				// update path_buf to have parent path produced by dirname()
				char path_buf_copy[PATH_MAX];
				snprintf(path_buf_copy, sizeof(path_buf_copy), "%s", path_buf);
				char *parent_path = dirname(path_buf_copy);
				memset(path_buf, 0, PATH_MAX);
				snprintf(path_buf, PATH_MAX, "%s", parent_path);

			}
			else {
				if (llapi_fid2path_at(mnt_fd, &rec->cr_pfid, path_buf, PATH_MAX, &recno, &linkno)) {
					perror("llapi_fid2path_at");
					return -1;
				}
			}

			// clean up path
			if (strcmp(path_buf, "/") == 0 || strcmp(path_buf, ".") == 0) {
				memset(path_buf, 0, PATH_MAX);
			}


			// print parent
			if (args.filename_set && dprintf(log_fd, "%s/%s\n", mnt_path, path_buf) < 0) {
				perror("dprintf");
				return -1;
			}

			printf("%s/%s\n", mnt_path, path_buf);

			if (linkno == linktmp) {
				break;
			}
			
			linktmp = linkno;

		}

		llapi_changelog_free(&rec);

	}

	if (args.filename_set) {
		// fsync() is only necessary if user specified a file; the default (stdout)
		// does not need syncing.
		int rc2 = fsync(log_fd);
		if (rc2) {
			fprintf(stderr, "fsync(%s) failed: %s\n", args.filename, strerror(errno));
			return -1;
		}
	}

	if (!args.keep_records && index != 0) {
		// llapi_changelog_clear() reports errors to stderr itself;
		// no need to report here, just return an error status.
		if (llapi_changelog_clear(args.mdt_name, args.consumer_name, index))
			return -1;
	}

	// llapi_changelog_recv() returns 1 for normal exit, so
	// invert that to return a standard 0 for success:
	return rc == 1 ? 0 : -1;
}

int main(int argc, char *argv[])
{
	void *changelog_priv;
	int rc = 0;
	int log_fd;
	int mnt_fd;
	long long startrec = 0;
	FILE* log_fp;

	if (parse_args(argc, argv)) {
		return EXIT_FAILURE;
	}

	
	
	// get file descriptor for lustre mount point 
	char* suffix = strstr(args.mdt_name, "-MDT");
	if (suffix == NULL) {
		printf("%s is not a valid MDT name", args.mdt_name);
		return -1;
	}

	// get mount path from mdt name 	
	int fs_name_len = suffix - args.mdt_name; 
	char* fs_name = calloc(fs_name_len + 1, sizeof(char));
	memcpy(fs_name, args.mdt_name, fs_name_len);
	
	char* mnt_path = calloc(PATH_MAX, sizeof(char));
	if (llapi_search_rootpath(mnt_path, fs_name)) {
		perror("llapi_search_rootpath");
	}

	free(fs_name);

	mnt_fd = open(mnt_path, O_RDONLY|O_DIRECTORY);
	if (mnt_fd < 0) {
		fprintf(stderr, "Error opening file %s: %s\n", mnt_path, strerror(errno));
		return EXIT_FAILURE;
	}

	log_fd = open(args.filename, O_RDWR|O_APPEND|O_CREAT, 0644);
	if (log_fd < 0) {
		fprintf(stderr, "Error opening file %s: %s\n", args.filename, strerror(errno));
		return EXIT_FAILURE;
	}

	if (args.filename_set) {
		// print all files from log before parsing new changelog entries 
		log_fp = fdopen(log_fd, "r");

		char *line = NULL;
		size_t cap = 0;

		while (getline(&line, &cap, log_fp) != -1) {
			    printf("%s", line);
		}

	}
	
	rc = llapi_changelog_start(&changelog_priv,
				   CHANGELOG_FLAG_JOBID |
				   CHANGELOG_FLAG_EXTRA_FLAGS,
				   args.mdt_name, startrec);
	if (rc < 0) {
		fprintf(stderr, "error starting changelog: %s\n", strerror(-rc));
		goto close;
	}

	rc = process_records(changelog_priv, mnt_fd, log_fd, mnt_path);

	llapi_changelog_fini(&changelog_priv);

close:
	close(mnt_fd);

	if (args.filename_set) {
		fclose(log_fp);
	}

	return rc ? EXIT_FAILURE : EXIT_SUCCESS;
}
