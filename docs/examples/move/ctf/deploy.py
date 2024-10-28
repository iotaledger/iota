import os
import glob
import subprocess
import sys
import json
import time
from pprint import pp

def execute_in_directories(executable_path, base_directory):
    # Check if the executable exists
    base_directory = os.path.abspath(base_directory)

    if not os.path.isfile(executable_path):
        print(f"Executable not found: {executable_path}")
        sys.exit(1)
    
    to_whitelist = []
    package = None
    cap = None
    challenges = None

    # Iterate over all directories in the base_directory
    for folder in sorted(glob.glob(os.path.join(base_directory, '*'))):
        if os.path.isdir(folder) and 'challenge_' in folder:
            print(folder.rsplit('/', 1)[1])
            try:
                # Change to the current directory
                os.chdir(folder)
                # Execute the command
                data = subprocess.run([executable_path, 'client', 'publish', '--json', '--gas-budget=100000000', '--skip-dependency-verification'], check=False, capture_output=True, text=True)
                #print(data.stdout)
                data.check_returncode()
                deployed = json.loads(data.stdout)
                for oc in deployed['objectChanges']:
                    if oc['type'] == 'published':
                        pid = oc['packageId']
                        modules = oc['modules']
                        print(f"Package: {pid}")

                        for module in modules:
                            to_whitelist.append('%s::%s::Flag' % (pid, module))

                    elif oc['type'] == 'created':
                        otype = oc['objectType'].split('::', 2)[2]
                        oid = oc['objectId']
                        print(f"{otype}: {oid}")

            except subprocess.CalledProcessError as e:
                print(f"Error executing in {folder}: {e}")
            except Exception as e:
                print(f"An unexpected error occurred in {folder}: {e}")
            finally:
                # Change back to the base directory
                os.chdir(base_directory)
                time.sleep(5)

        elif os.path.isdir(folder) and 'verifier' in folder:
            try:
                # Change to the current directory
                os.chdir(folder)
                # Execute the command
                data = subprocess.run([executable_path, 'client', 'publish', '--json', '--gas-budget=100000000', '--skip-dependency-verification'], check=True, capture_output=True, text=True)
                deployed = json.loads(data.stdout)
                for oc in deployed['objectChanges']:
                    if oc['type'] == 'published':
                        package = oc['packageId']
                    elif oc['type'] == 'created':
                        if oc['objectType'].endswith('CTFCap'):
                            cap = oc['objectId']
                        if oc['objectType'].endswith('Challenges'):
                            challenges = oc['objectId']

            except subprocess.CalledProcessError as e:
                print(f"Error executing in {folder}: {e}")
            except Exception as e:
                print(f"An unexpected error occurred in {folder}: {e}")
            finally:
                # Change back to the base directory
                os.chdir(base_directory)
                time.sleep(5)

            print("Done")

        
    print("Adding allowed flags to the verifier contract, false positives will fail")
    for module in to_whitelist:
        try:
            # iota client call --function allow_flag_type --module verifier --package $PACKAGE --args $CAP $SB --type-args 0xe6d41a7f9530acb58fae2318f33eb27f80f84406268a23043ab9289d318f3ddc::checkin::Flag --gas-budget 100000000
            subprocess.run([executable_path, 'client', 'call', '--function', 'allow_flag_type', '--module', 'verifier', '--package', package, '--args', cap, challenges, '--type-args', module, '--gas-budget', '100000000'], check=True, capture_output=True, text=True)
        except Exception as e:
            print(f"Failed to allow flag `{module}`, it probably does not exist (false positive)")

    print('All Done!\n')
    print('-' * 80)
    print('Package ID for Verifier: %s' % package)
    print('CTFCap Object ID: %s' % cap)
    print('Shared Challenges Object ID: %s' % challenges)
    print('-' * 80)
            

if __name__ == "__main__":
    if len(sys.argv) != 3:
        print("Usage: python deploy.py <executable_path> <base_directory>")
        sys.exit(1)

    executable_path = sys.argv[1]
    base_directory = sys.argv[2]

    execute_in_directories(executable_path, base_directory)

