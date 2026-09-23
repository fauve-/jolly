#include <cstdio>
#include <cuda_runtime.h>

#define CUDA_CHECK(call)                                              \
    do {                                                              \
        cudaError_t err = (call);                                     \
        if (err != cudaSuccess) {                                     \
            fprintf(stderr, "%s:%d: CUDA error: %s\n",                \
                   __FILE__, __LINE__, cudaGetErrorString(err));      \
	    return 1;                                                 \
        }                                                             \
    } while (0)

__global__ void hello()
{
    printf("Hello from GPU thread %d\n", threadIdx.x);
}

int main()
{
    int count = 0;
    CUDA_CHECK(cudaGetDeviceCount(&count));

    printf("CUDA devices: %d\n", count);

    if (count == 0)
        return 1;

    cudaDeviceProp prop{};
    CUDA_CHECK(cudaGetDeviceProperties(&prop, 0));

    printf("GPU: %s\n", prop.name);
    printf("Compute capability: %d.%d\n", prop.major, prop.minor);

    hello<<<1, 8>>>();

    CUDA_CHECK(cudaGetLastError());
    CUDA_CHECK(cudaDeviceSynchronize());

    return 0;
}