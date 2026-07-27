ec-slimloader implementation for mcxa for signed images.

The bootloader expects an two slots in internal flash and two in external flash.
The A and B of the internal flash will be swapped and the A and B of the external flash will be swapped too.
All slots must contain a signed NXP image.
