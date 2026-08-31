/*
 * Kog PSF helper access to libupse's internal timing switch.
 * Copyright (C) 2026 Kog contributors.
 * SPDX-License-Identifier: GPL-2.0-only
 */

#include "upse-internal.h"

void kog_upse_disable_stop(upse_module_t *module)
{
    upse_ps1_spu_setlength(module->instance.spu, 0, 0);
}
