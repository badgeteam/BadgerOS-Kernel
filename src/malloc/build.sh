set -e

defines="-DBADGEROS_MALLOC_STANDALONE -DBADGEROS_MALLOC_DEBUG_LEVEL=3"
sources="main.c static-buddy.c slab-alloc.c"

echo "64-bit"
gcc -m64 -g3 -Wall -Wextra ${defines} ${sources} -o main64
echo "32-bit"
gcc -m32 -g3 -Wall -Wextra ${defines} ${sources} -o main32
echo "64-bit softbit"
gcc -DSOFTBIT -g3 -Wall -Wextra ${defines} ${sources} -o main64-softbit
echo "32-bit softbit"
gcc -DSOFTBIT -m32 -g3 -Wall -Wextra ${defines} ${sources} -o main32-softbit

echo "wrapper"
gcc -std=gnu17 -g3 -Wall -Wextra -DPRELOAD ${sources} ${defines} malloc.c -Wl,--wrap,malloc -Wl,--wrap,free -Wl,--wrap,calloc -Wl,--wrap,realloc -Wl,--wrap,reallocarray -Wl,--wrap,aligned_alloc -Wl,--wrap,posix_memalign -fpic -shared -o malloc.so

#riscv64-linux-gnu-gcc -g3 -Wall -Wextra ${defines} ${sources} -o mainrv64
#riscv64-linux-gnu-gcc -march=rv32imac_zicsr_zifencei -g3 -Wall -Wextra ${defines} ${sources} -o mainrv32
