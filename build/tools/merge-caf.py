#!/usr/bin/env python3
#
#
# Copyright (C) 2021 Paranoid Android
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#      http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

"""
Merge script for Flamingo OS. All thanks to AOSPA

 The source directory; this is automatically two folder up because the script
 is located in vendor/flamingo/build/tools. Other ROMs will need to change this. The logic is
 as follows:

 1. Get the absolute path of the script with os.path.realpath in case there is a symlink
    This script may be symlinked by a manifest so we need to account for that
 2. Get the folder containing the script with dirname
 3. Move into the folder that is three folders above that one and print it

"""

import argparse
import os
import re
import shutil
import subprocess
import xml.etree.ElementTree as Et

import git
from git.exc import GitCommandError
from git import Repo

BASE_URL = "https://git.codelinaro.org/clo/la/"
WORKING_DIR = "{0}/../../../..".format(os.path.dirname(os.path.realpath(__file__)))
MANIFEST_NAME = "flamingo.xml"
REPOS_TO_MERGE = {}
REPOS_RESULTS = {}
BRANCH_NAME = "A12.1"


# useful helpers
def nice_error():
    """ Errors out in a non-ugly way. """
    print("Invalid repo, are you sure this repo is on the tag you're merging?")


def get_manual_repos(args, is_system):
    """ Get all manually (optional) specified repos from arguments """
    ret_lst = {}
    default_repos = list_default_repos(is_system)
    if args.repos_to_merge:
        for repo in args.repos_to_merge:
            if repo not in default_repos:
                nice_error()
                return None, None
            ret_lst[repo] = default_repos[repo]
    return ret_lst, default_repos


def list_default_repos(is_system):
    """ Gathers all repos from split system.xml and vendor.xml """
    default_repos = {}
    if is_system:
        with open(
            "{0}/.repo/manifests/system.xml".format(WORKING_DIR)
        ) as system_manifest:
            system_root = Et.parse(system_manifest).getroot()
            for child in system_root:
                path = child.get("path")
                if path:
                    default_repos[path] = child.get("name")
    else:
        with open(
            "{0}/.repo/manifests/vendor.xml".format(WORKING_DIR)
        ) as vendor_manifest:
            vendor_root = Et.parse(vendor_manifest).getroot()
            for child in vendor_root:
                path = child.get("path")
                if path:
                    default_repos[path] = child.get("name")
    return default_repos


def read_custom_manifest(default_repos):
    """ Finds all repos that need to be merged """
    print("Finding repos to merge...")
    removed_repos = []
    project_repos = []
    reversed_default = {value: key for key, value in default_repos.items()}

    with open("{0}/.repo/manifests/remove.xml".format(WORKING_DIR)) as remove_manifest:
        root = Et.parse(remove_manifest).getroot()
        for repo in root:
            if repo.tag == "remove-project":
                removed_repos.append(repo.get("name"))

    with open("{0}/.repo/manifests/{1}".format(WORKING_DIR, MANIFEST_NAME)) as manifest:
        root = Et.parse(manifest).getroot()
        for repo in root:
            if repo.get("remote") == "flamingo":
                    project_repos.append(repo.get("path"))

        for repo in removed_repos:
            if repo in reversed_default:
                if reversed_default[repo] in project_repos:
                    REPOS_TO_MERGE[reversed_default[repo]] = repo


def force_sync(repo_lst):
    """ Force syncs all the repos that need to be merged """
    print("Syncing repos")
    for repo in repo_lst:
        if os.path.isdir("{}{}".format(WORKING_DIR, repo)):
            shutil.rmtree("{}{}".format(WORKING_DIR, repo))

    cpu_count = str(os.cpu_count())
    args = [
        "repo",
        "sync",
        "-c",
        "--force-sync",
        "--no-clone-bundle",
        "--no-tag",
        "-j",
        cpu_count,
        "-q",
    ] + list(repo_lst.values())
    subprocess.run(
        args,
        check=False,
    )


def merge(repo_lst, branch):
    """ Merges the necessary repos and lists if a repo succeeds or fails """
    failures = []
    successes = []
    for repo in repo_lst:
        print("Merging " + repo)
        os.chdir("{0}/{1}".format(WORKING_DIR, repo))
        try:
            git.cmd.Git().pull("{}{}".format(BASE_URL, repo_lst[repo]), branch)
            if check_actual_merged_repo(repo, branch):
                successes.append(repo)
        except GitCommandError as git_error:
            print(git_error)
            failures.append(repo)

    REPOS_RESULTS.update({"Successes": successes, "Failures": failures})


def merge_manifest(is_system, branch):
    """ Updates CAF revision in .repo/manifests """
    with open("{0}/.repo/manifests/default.xml".format(WORKING_DIR)) as manifestxml:
        tree = Et.parse(manifestxml)
        root = tree.getroot()
        lst = root.findall("remote")
        remote = None
        for elem in lst:
            if is_system and elem.attrib["name"] == "clo_system":
                remote = elem
                break
            elif elem.attrib["name"] == "clo_vendor":
                remote = elem
                break
        remote.set("revision", branch)
        tree.write("{0}/.repo/manifests/default.xml".format(WORKING_DIR))
        cpu_count = str(os.cpu_count())
        subprocess.run(
            [
                "repo",
                "sync",
                "-c",
                "--force-sync",
                "--no-clone-bundle",
                "--no-tag",
                "-j",
                cpu_count,
                "-q",
                "-d",
            ],
            check=False,
        )
        git_repo = git.Repo("{0}/.repo/manifests".format(WORKING_DIR))
        git_repo.git.add("*")
        git_repo.index.commit(f"manifest: Update to {branch}")


def check_actual_merged_repo(repo, branch):
    """Gets all the repos that were actually merged and
    not the ones that were just up-to-date"""
    git_repo = git.Repo("{0}/{1}".format(WORKING_DIR, repo))
    commits = list(git_repo.iter_commits("HEAD", max_count=1))
    result = commits[0].message
    if branch.split("/")[2] in result:
        return True
    return False


def print_results(branch):
    """ Prints all all repos that will need to be manually fixed """
    if REPOS_RESULTS["Failures"]:
        print("\nThese repos failed to merge, fix manually: ")
        for failure in REPOS_RESULTS["Failures"]:
            print(failure)
    if REPOS_RESULTS["Successes"]:
        print("\nRepos that merged successfully: ")
        for success in REPOS_RESULTS["Successes"]:
            print(success)
    print()
    if not REPOS_RESULTS["Failures"] and REPOS_RESULTS["Successes"]:
        print(
            "{0} merged successfully! Compile and test before pushing to GitHub.".format(
                branch.split("/")[2]
            )
        )
    elif not REPOS_RESULTS["Failures"] and not REPOS_RESULTS["Successes"]:
        print("Unable to retrieve any results")


def push_repos():
    """
    Push all the merged repos to our git
    """
    for repo_dir in REPOS_RESULTS["Successes"]:
        try:
            print(f"Pushing {repo_dir}")
            repo = Repo(f"{WORKING_DIR}/{repo_dir}")
            origin = repo.remote(name='flamingo')
            assert origin.exists()
            origin.push(f'HEAD:{BRANCH_NAME}').raise_if_error()   
        except:
            print(f"Failed to push {repo_dir}")

def bump_version():
    """
    Bump FLAMINGO_VERSION_MINOR whenever a new SPL is merged
    """
    MAJOR_VERSION_STRING_PATTERN = "FLAMINGO_VERSION_MAJOR := \\d+"
    MINOR_VERSION_STRING_PATTERN = "FLAMINGO_VERSION_MINOR := \\d+"

    file_path = f"{WORKING_DIR}/vendor/flamingo/target/product/version.mk"
    fin = open(file_path, "rt")
    data = fin.read()
    fin.close()
    major_ver_str = re.findall(MAJOR_VERSION_STRING_PATTERN, data)[0]
    major_ver = int(major_ver_str.partition(":= ")[-1])
    old_minor_ver_str = re.findall(MINOR_VERSION_STRING_PATTERN, data)[0]
    minor_ver = int(old_minor_ver_str.partition(":= ")[-1])
    new_minor_ver_str = old_minor_ver_str.replace(str(minor_ver), str(minor_ver + 1))
    data = data.replace(old_minor_ver_str, new_minor_ver_str)

    fout = open(file_path, "wt")
    fout.write(data)
    fout.close()

    repo = Repo(f"{WORKING_DIR}/vendor/flamingo")
    repo.git.add("target/product/version.mk")
    
    repo.index.commit(f"flamingo: Bump version to {major_ver}.{minor_ver + 1}")
    origin = repo.remote(name='flamingo')
    assert origin.exists()
    origin.push(f'HEAD:{BRANCH_NAME}').raise_if_error()
     

def main():
    """Gathers and merges all repos from CAF and
    reports all repos that need to be fixed manually"""

    parser = argparse.ArgumentParser(description="Merge a CAF revision.")
    parser.add_argument(
        "branch_to_merge",
        metavar="branch",
        type=str,
        help="a tag to merge from source.codeaurora.org",
    )
    parser.add_argument(
        "--repos",
        dest="repos_to_merge",
        nargs="*",
        type=str,
        help="path of repos to merge",
    )
    parser.add_argument(
        "--merge-manifest",
        dest="merge_manifest",
        action="store_true",
        help="automatically update manifest before merging repos",
    )
    parser.add_argument(
        "--dry-run",
        dest="dry_run",
        action="store_true",
        help="Dry run the merge script (for testing purposes)",
    )
    parser.add_argument(
        "--push",
        dest="push_repos",
        action="store_true",
        help="Push all your merged repos after testing"
    )
    parser.add_argument(
        "--bump",
        dest="bump_version",
        action="store_true",
        help="Bump up min version number of Flamingo"
    )
    parser.add_argument(
        "--no-sync",
        dest="no_sync",
        action="store_false",
        help="Skip force syncing all the repos"
    )
    args = parser.parse_args()

    branch = "refs/tags/{}".format(args.branch_to_merge)

    is_system = "QSSI" in branch
    repo_lst, default_repos = get_manual_repos(args, is_system)
    if repo_lst is None and default_repos is None:
        return
    if len(repo_lst) == 0:
        read_custom_manifest(default_repos)
        if args.dry_run:
            print(list(REPOS_TO_MERGE.keys()))
            quit()
        if REPOS_TO_MERGE:
            if args.merge_manifest:
                merge_manifest(is_system, branch)
            if not args.no_sync:
                force_sync(REPOS_TO_MERGE)
            merge(REPOS_TO_MERGE, branch)
            os.chdir(WORKING_DIR)
            print_results(branch)
        else:
            print("No repos to sync")
    else:
        force_sync(repo_lst)
        merge(repo_lst, branch)
        os.chdir(WORKING_DIR)
        print_results(branch)

    if(args.push_repos):
        push_repos()

    if(args.bump_version):
        bump_version()

if __name__ == "__main__":
    # execute only if run as a script
    main()
