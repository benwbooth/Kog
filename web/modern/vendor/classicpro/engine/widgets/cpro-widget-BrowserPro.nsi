;###########################################################################################
;###########################################################################################
;#
;#								   ClassicPro Widget Installer         
;#							   Copyright (c) 2009 by SkinConsortium   
;#
;###########################################################################################
;###########################################################################################


;-------------------------------------------------------------------------------------------
; Change your widget information here
;-------------------------------------------------------------------------------------------

; The name of your widget
!define CPRO_WIDGET_NAME "BrowserPro"

; The current version of your widget
!define CPRO_WIDGET_VERSION "1.0"

; The current revision of your widget
!define CPRO_WIDGET_REVISION "0"
	
; The current build of your widget	
!define CPRO_WIDGET_BUILD "0"
	
; The XML file used as entry point for your widget. Located in /Load
!define CPRO_WIDGET_XMLFILENAME "browserpro.xml"

; The data folder of your widget. Located in /Data
!define CPRO_WIDGET_DATA_FOLDERNAME "BrowserPro"

; The filename of this installation script
!define CPRO_WIDGET_NSISFILENAME "cpro-widget-BrowserPro.nsi"

; Your ClassicPro Directory
!define CPRO_WIDGET_SOURCEPATH "C:\Program Files\Winamp\Plugins\ClassicPro"

; The name of cPro Widget uninstaller
!define CPRO_WIDGET_UNINSTALLER "Uninstall (${CPRO_WIDGET_NAME}).exe"

; Your Widget setup file directory (use empty path to create it in script directory)	
!define CPRO_WIDGET_OUTFILE_PATH ""

; Branding Text URL 
!define CPRO_WIDGET_BT "http://cpro.skinconsortium.com"
	
; Shows (1) / hides (0) the components page of installer	
!define CPRO_WIDGET_SHOW_COMPONENTS_PAGE "0"	
	
; The PayPal link for widget author (by default SkinConsortium account)	
!define CPRO_WIDGET_PAYPAL_LINK "https://www.paypal.com/uk/cgi-bin/webscr?cmd=_flow&SESSION=lbtyhrWugcvcf_QcrMnTrArKKiT3DcYJbH-_gFqC8-fXZNwJ4ibp2UbTunS&dispatch=5885d80a13c0db1fa798f5a5f5ae42e779d4b5655493f6171509c5b2ec019b86"

; CPro widget web page
!define CPRO_WIDGET_WEB_PAGE "http://cpro.skinconsortium.com"

; Web page with widget support
!define CPRO_WIDGET_HELP_LINK "http://forums.skinconsortium.com/index.php?page=Thread&threadID=1084"

; Author of this widget
!define CPRO_WIDGET_AUTHOR "Pieter Niewoudt"

; Widget company
!define CPRO_WIDGET_COMPANY "Skin Consortium"

; Widget copyright
!define /Date CPRO_WIDGET_COPYRIGHT "Copyright (c) 2005-%Y"	

; Winamp version needed to run CPro and this widget 
!define CPRO_WINAMP_VERSION "5.55"

; The name of CPro 
!define CPRO_NAME "ClassicPro"

; Copyright
!define CPRO_CRS "©"	

; Current version of ClassicPro 
!define CPRO_VERSION "1.1"

;-------------------------------------------------------------------------------------------
; Always include this file
;-------------------------------------------------------------------------------------------
!include "${CPRO_WIDGET_SOURCEPATH}\_installer\cPro_Widget_Installer.nsh"