import {
  useMutation,
  useQuery,
  useQueryClient,
  type UseMutationOptions,
  type UseQueryOptions,
} from "@tanstack/react-query"

import { apiClient } from "@/lib/api/client"
import {
  isTerminalScanStatus,
  type CreateScanRequest,
  type CreateScanResponse,
  type ScanResponse,
} from "@/lib/api/types"

export const scanQueryKeys = {
  all: ["scan"] as const,
  detail: (scanId: number) => [...scanQueryKeys.all, scanId] as const,
}

export function useCreateScan(
  options?: UseMutationOptions<CreateScanResponse, Error, CreateScanRequest>,
) {
  const queryClient = useQueryClient()

  return useMutation({
    ...options,
    mutationFn: (request) => apiClient.createScan(request),
    onSuccess: (data, variables, onMutateResult, context) => {
      void queryClient.invalidateQueries({
        queryKey: scanQueryKeys.detail(data.id),
      })

      options?.onSuccess?.(data, variables, onMutateResult, context)
    },
  })
}

type UseScanOptions = Omit<
  UseQueryOptions<ScanResponse, Error, ScanResponse, ReturnType<typeof scanQueryKeys.detail>>,
  "queryKey" | "queryFn" | "enabled" | "refetchInterval"
>

export function useScan(scanId: number | null, options?: UseScanOptions) {
  return useQuery({
    queryKey: scanQueryKeys.detail(scanId ?? 0),
    queryFn: () => {
      if (scanId === null) {
        throw new Error("scanId is required to fetch a scan.")
      }

      return apiClient.getScan(scanId)
    },
    enabled: scanId !== null,
    retry: scanId === null ? false : 20,
    retryDelay: 1_000,
    refetchInterval: (query) => {
      const scan = query.state.data

      if (scan === undefined) {
        return 1_500
      }

      return isTerminalScanStatus(scan.status) ? false : 1_500
    },
    ...options,
  })
}
