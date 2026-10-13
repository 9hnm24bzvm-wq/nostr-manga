# Local runnable backend demo templates

The executable manga-demo is built from server/src/bin/manga-demo.rs. It exercises real two-node publication, original reads, approved peer sync, a public-library save and cold recovery. It refuses non-loopback origins. All demonstration keys are public synthetic fixtures: never expose this demo on the Internet.

run-demo.sh expects a prepared runtime directory containing bin/manga-node, bin/manga-demo, bin/strfry and postgres/{bin,lib,share}. Use the separately supplied Linux x86_64 runtime package, or build the application and prepare these official runtimes yourself. This source directory alone is not the bundled executable package. No dependencies or credentials are downloaded by run-demo.sh.

Run as a non-root user with Python3 and the required system shared libraries. ./run-demo.sh retains loopback servers and temporary data until interrupted; ./run-demo.sh --check exits after assertions and removes only its successful generated temporary directory. Failed-run logs remain for inspection.

node.example.json is an operator configuration template. Replace placeholders deliberately, use private service keys, an appropriately secured database and HTTPS before any public deployment. This demonstration does not configure or certify production deployment or target capacity.
