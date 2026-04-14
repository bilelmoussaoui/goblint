#pragma once

#include <glib-object.h>

/* Single-line without semicolon */
G_DECLARE_FINAL_TYPE (FooBar, foo_bar, FOO, BAR, GObject)

/* Multi-line without semicolon - case 1 */
G_DECLARE_FINAL_TYPE (KioskApp,
                      kiosk_app,
                      KIOSK,
                      APP,
                      GObject)

/* Multi-line without semicolon - case 2 */
G_DECLARE_FINAL_TYPE (KioskAreaConstraint, kiosk_area_constraint,
                      KIOSK, AREA_CONSTRAINT, GObject)

/* Already correct - has semicolon */
G_DECLARE_DERIVABLE_TYPE (CorrectType, correct_type,
                          CORRECT, TYPE, GObject);

/* Multi-line interface without semicolon */
G_DECLARE_INTERFACE (MyInterface,
                     my_interface,
                     MY,
                     INTERFACE,
                     GObject)
