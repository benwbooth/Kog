#include <lib/std.mi>
#include <lib/pldir.mi>
#include <lib/colormgr.mi>
#include ../../../../two/scripts/attribs/init_Playlist.m
//#include <lib/config.mi>
// this is the page that maps its items to the windows menu (aka View), you can add attribs or more pages (submenus)
//#define CUSTOM_WINDOWSMENU_ITEMS "{6559CA61-7EB2-4415-A8A9-A2AEEF762B7F}"

Global ColorMgr StartupCallback;

#define VIDEO_GUID "{F0816D7B-FFFC-4343-80F2-E8199AA15CC3}"
#define VIS_GUID "{0000000A-000C-0010-FF7B-01014263450C}"
#define ML_GUID "{6B0EDF80-C9A5-11D3-9F26-00C04F39FFC6}"
#define CE_GUID "{00000000-0000-0000-0000-000000000000}"
#define CON_GUID "{7383A6FB-1D01-413B-A99A-7E6F655F4591}"
#define LIR_GUID "{7A8B2D76-9531-43B9-91A1-AC455A7C8242}"
#define DL_GUID "{A3EF47BD-39EB-435A-9FB3-A5D87F6F17A5}"
#define PL_GUID "{45F3F7C1-A6F3-4EE6-A15E-125E92FC3F8D}"
#define QUE_MAN "{F8CD22A4-0C33-45FB-A360-70D7240FC23E}"
#define JTF_GUID "{5F8D8373-EAA7-4390-B5AB-402E86A5F9DD}"
#define GUID_BLACKLIST "{D6201408-476A-4308-BF1B-7BACA1124B12};{5F8D8373-EAA7-4390-B5AB-402E86A5F9DD};{00000000-0000-0000-0000-000000000000}"
#define DEF_DRAWER_H 119

#define DRAWER_PL_ID 4
#define DRAWER_VID_ID 5
#define DRAWER_VIS_ID 6
#define MINI_VID_ID 3
#define MINI_VIS_ID 4


#define WIDGET_TAB_ID -666
#define myDelay 10

//{00000010-0000-00FF-8000-C5E2FB8CD50B}
//{00000011-0000-00FF-8000-C5E2FB8CD50B}
//{00000012-0000-00FF-8000-C5E2FB8CD50B}
//{00000013-0000-00FF-8000-C5E2FB8CD50B}

// Functions used
Function openMainLayoutNow();
Function setDrawer(boolean onOff);
Function openTabNo(int tabNo);
Function openWidgetIDS(String ids);
Function openMini(int miniNo);
Function refreshComponentButtons();
Function setMainFrame(boolean open);
//Function setCompStatus(boolean onOff);
//Function updateCompStatus();
Function setFrame1();
Function setFrame2(int pos, int h);
Function gotoPrevMini();
Function gotoNextMini();
//Function setMiniBG(int mode); //0=normal, 1=tagview
Function setDrawerSizeSave(int h);

// Main Layout
Global Container player;
Global Layout normal;
Global Group xuiGroup;

// Mini Area stuff
Global Group mini_Cover, mini_Video, mini_AVS, mini_SavedPL, mini_TagView;
Global Button but_miniGoto;
Global PopUpMenu popMenu;
Global Boolean mouse_but_miniGoto,  dmr_down;
Global GuiObject gad_Grid;
Global Layer gadgrid1, gadgrid1a, gadgrid1a_overlay, gadgrid2;

// Gui Extras
Global Text plText1, plText2;
Global GuiObject gridPL;
Global Button plHideSearch, plHideSearchIcon;

// Drawer stuff
Global boolean open_drawer, busyWithDrawer, cuseqbg;
Global ComponentBucket dummyBuck;
Global GuiObject customObj;
Global ToggleButton tog_drawer, tog_search;
Global Group drawer, drawer_pl, mainTabsheet;

// Wasabi:Frame stuff
Global Frame mainFrame, plFrame;
Global boolean mouseDownF1, mouseDownF2, stopResizeRight;
Global Button closeFrame, openFrame;
Global Group area_left, area_right, area_mini, area_right_pl, ocFrame;//, CproTabs;

// Component Handling
Global int delayStartTab, widgetStatus;
Global boolean dontTabCall, skipLoad, busyWithThisFunction, wasTabTrig, delayStart, ml_installed;
Global String closeGUID, thirdPartyGuid;
Global int active_tab, tab_openned, delayStartTab;
Global Timer openMainLayout, openDefaultTab, refreshAIOTab, ssWinHol, openMiniDelay, checkVisName;
Global WindowHolder hold_Other, hold_Pl2, hold_vid, hold_avs, hold_ml;
Global WindowHolder hold_avs_mini, hold_avs_drawer, hold_vid_mini, hold_vid_drawer;
Global Text visName;
Global GuiObject guihold_Pl2, compGrid;
Global GuiObject visRectBg, CproTabs;
Global Browser xuiBrowser;
Global Group tabbut_vid, tabbut_avs, tabbut_pl;
Global Group tab_library, tab_video, tab_avs, tab_Browser, tab_Playlist, tab_Other, tab_Widget;
Global GuiObject widgetHolder, drawerResize;
Global int _powerSave1, _powerSave2, _powerSave3, _powerSave4;

//Fade cover status
Global GuiObject coverStatusGrid;
Global Layer wasabiCover;
Global Boolean menuOpen;

Global ConfigItem custom_windows_page;
Global ConfigAttribute sui_vis_attrib;
Global Boolean myChange, openPlayerFromNow;


System.onScriptLoaded(){
	StartupCallback = new ColorMgr;
	dontTabCall=false;
	delayStart=true;
	
	delayStartTab = 0;
	widgetStatus = 0;
	tab_openned = -1;
	active_tab = getPublicInt("cpro2.lastComponentPage", 0);
	ml_installed = stringToInteger(getParam());
	open_drawer=false;
	skipLoad=true;

	initAttribs_Playlist();

	// Grab all the objects
	player = getContainer("main");
	normal = player.getLayout("normal");
	xuiGroup = getScriptGroup();
	mainTabsheet = xuiGroup.findObject("centro.componentsheet");
	mainFrame = xuiGroup.findObject("centro.mainframe");
	plFrame = xuiGroup.findObject("centro.plframe");
	ocFrame = xuiGroup.findObject("centro.componentsheet.opencloseframe");
	CproTabs = xuiGroup.findObject("Cpro.tabs");
	visName = xuiGroup.findObject("centro.visname");
	closeFrame = xuiGroup.findObject("close.rightframe");
	openFrame = xuiGroup.findObject("open.rightframe");
	drawer = xuiGroup.findObject("centro.multidrawer");
	drawer_pl = drawer.getObject("drawer.playlist");
	compGrid = xuiGroup.findObject("centro.componentsheet.grid");
	but_miniGoto = xuiGroup.findObject("comp.goto");
	mini_Cover = xuiGroup.findObject("centro.playlist.directory.cov");
	mini_Video = xuiGroup.findObject("centro.playlist.directory.vid");
	mini_AVS = xuiGroup.findObject("centro.playlist.directory.vis");
	mini_SavedPL = xuiGroup.findObject("centro.playlist.directory.spl");
	mini_TagView = xuiGroup.findObject("centro.playlist.directory.tag");
	area_right_pl = xuiGroup.findObject("centro.playlist.component"); 
	plText1 = area_right_pl.getObject("centro2.group.pl.buttons").findObject("centro.playlist.pltext1");
	gridPL = area_right_pl.getObject("centro2.group.pl.buttons").findObject("centro.playlist.textgrid");
	plHideSearch = area_right_pl.getObject("centro2.group.pl.buttons").findObject("pl.search.toggle");
	plHideSearchIcon = area_right_pl.getObject("centro2.group.pl.buttons").findObject("pl.search.toggle.overlay");
	plText2 = xuiGroup.findObject("centro.playlist.pltext2");
	tabbut_vid = xuiGroup.findObject("centro.video.buttons");
	tabbut_avs = xuiGroup.findObject("centro.visualization.buttons");
	tab_library = xuiGroup.findObject("centro.library");
	tab_video = xuiGroup.findObject("centro.video");
	tab_avs = xuiGroup.findObject("centro.visualization");
	tab_Browser = xuiGroup.findObject("centro.browser");
	xuiBrowser = xuiGroup.findObject("browserpro.browser");
	tab_Playlist = xuiGroup.findObject("centro.playlist2");
	tabbut_pl = tab_Playlist.getObject("centro2.group.pl.buttons");
	tab_Other = xuiGroup.findObject("centro.other");
	tab_Widget = xuiGroup.findObject("centro.widget");
	widgetHolder = tab_Widget.findObject("widget.holder");
	tog_drawer = xuiGroup.findObject("tog.drawer");
	area_left = xuiGroup.findObject("centro.components");
	area_right = xuiGroup.findObject("centro.playlist1");
	area_mini = xuiGroup.findObject("centro.playlist.directory");
	hold_Pl2 = xuiGroup.findObject("centro.windowholder.playlist2");
	guihold_Pl2 = xuiGroup.findObject("centro.windowholder.playlist2");
	hold_vid = xuiGroup.findObject("centro.windowholder.video");
	hold_avs = xuiGroup.findObject("centro.windowholder.visualization");
	
	hold_avs_mini = mini_AVS.getObject("small.windowholder.visualization");
	hold_avs_drawer = xuiGroup.findObject("drawer.avs").findObject("small.windowholder.visualization");
	hold_vid_mini = mini_Video.getObject("small.windowholder.video");
	hold_vid_drawer = xuiGroup.findObject("drawer.video").findObject("small.windowholder.video");
	
	hold_ml = xuiGroup.findObject("centro.windowholder.library");
	visRectBg = xuiGroup.findObject("centro.windowholder.visualization.bg");
	dummyBuck = xuiGroup.findObject("widget.loader.mini");
	customObj = xuiGroup.findObject("widget.holder.mini");
	hold_Other = xuiGroup.findObject("centro.windowholder.other");
	gad_Grid = xuiGroup.findObject("centro.mini.grid");
	gadgrid1 = xuiGroup.findObject("centro.bottomleftgrid.1");
	gadgrid1a = xuiGroup.findObject("centro.bottomleftgrid.1a");
	gadgrid1a_overlay = xuiGroup.findObject("centro.bottomleftgrid.1a.overlay");
	gadgrid2 = xuiGroup.findObject("centro.bottomleftgrid.2");
	drawerResize = xuiGroup.findObject("cpro.drawer.resize");

	coverStatusGrid = xuiGroup.findObject("centro.playlist.wasabicover.status");
	wasabiCover = xuiGroup.findObject("centro.playlist.wasabicover");
	//but_miniGoto coverStatusGrid
	
	//Playlist Search box
	tog_search = xuiGroup.findObject("pl.search.toggle");
	
	
	/*tog_search = tab_Playlist.getObject("centro2.group.pl.buttons").getObject("pl.search.toggle"); //Main Screen
	tog_search = area_right_pl.getObject("centro2.group.pl.buttons").getObject("pl.search.toggle"); //Regs PL
	tog_search = xuiGroup.findObject("drawer.playlist").getObject("centro2.group.pl.buttons").getObject("pl.search.toggle"); //Drawer PL
	tog_search.setActivated(getPublicInt("Cpro2.plsearch", 1));*/

	// Check to see if the new eq background is used
	Map myMap = new Map;
	myMap.loadMap("read.suiframe.png");
	if(myMap.getWidth()>=272) cuseqbg=true;
	else  cuseqbg=false;
	delete myMap;
	
	//cpro2.size.status.text
	
	custom_windows_page = Config.getItem(CUSTOM_WINDOWSMENU_ITEMS);
	sui_vis_attrib = custom_windows_page.newAttribute("Visualizations\tCtrl+Shift+K", "0");


	// Timers
	openMainLayout = new Timer;
	openDefaultTab = new Timer;
	refreshAIOTab = new Timer;
	checkVisName = new Timer;
	ssWinHol = new Timer;
	openMiniDelay = new Timer;
	openMainLayout.setDelay(myDelay);
	openDefaultTab.setDelay(myDelay);
	refreshAIOTab.setDelay(myDelay);
	checkVisName.setDelay(500);
	ssWinHol.setDelay(66);
	openMiniDelay.setDelay(66);
	
	//Saved Settings
	openMini(getPublicInt("cpro2.lastMini", 0));
	setDrawer(getPublicInt("cpro2.draweropened", 1));
	openWidgetIDS(getPublicString("cpro2.lastMainWidgetIDS", ""));
	refreshComponentButtons();
}
System.onscriptunloading(){
	delete openMainLayout;
	delete openDefaultTab;
	delete refreshAIOTab;
	delete checkVisName;
	delete ssWinHol;
	delete openMiniDelay;
}






wasabiCover.onEnterArea(){
	but_miniGoto.cancelTarget();
	coverStatusGrid.cancelTarget();
	but_miniGoto.setAlpha(255);
	coverStatusGrid.setAlpha(255);
}
wasabiCover.onLeaveArea(){
	if(menuOpen || getPublicInt("cpro2.lastMini", 1)!=0) return;
	but_miniGoto.setTargetA(0);
	coverStatusGrid.setTargetA(0);
	but_miniGoto.setTargetSpeed(0.3);
	coverStatusGrid.setTargetSpeed(0.3);
	but_miniGoto.gotoTarget();
	coverStatusGrid.gotoTarget();
}
but_miniGoto.onEnterArea(){
	mouse_but_miniGoto=true;
	
	if(getPublicInt("cpro2.lastMini", 0)!=0) return;
	wasabiCover.onEnterArea();
}
but_miniGoto.onLeaveArea(){
	mouse_but_miniGoto=false;
	
	if(getPublicInt("cpro2.lastMini", 0)!=0) return;
	wasabiCover.onLeaveArea();
}

xuiGroup.onSetVisible(boolean onOff){
	//debug("onSetVisible");
	int pageNo = getPublicInt("cpro2.lastComponentPage", 0);

	if(skipLoad && 	!wasTabTrig){
		//if(pageNo==5) pageNo=0;
		openTabNo(pageNo);
		//debugstring(integertoString(pageNo),9);
		ssWinHol.start();
		skipLoad=false;
	}
	if(mainFrame.getPosition()==0){
		area_right.hide();
	}
	
	if(!onOff){
		if(pageNo==5) openTabNo(0);
	}
}


System.onSetXuiParam(String param, String value) {
	if(strlower(param) == "enable_drawer"){
		setDrawer(stringToInteger(value));
	}
}

System.onOpenUrl(string url){
	openTabNo(4);
	xuiBrowser.sendAction ("openurl", url, 0, 0, 0, 0);
	return 1;
}

System.onGetCancelComponent(String guid, boolean goingvisible){
	//if(goingvisible) debugString("+" + guid, 9);
	//else debugString("-" + guid, 9);

	//debugString(guid,9);
	//setClipboardText(guid);
	//debug(guid);

	// Check to see if this component is on the blacklist, and if it is, it will open in its own window or just close it...
	for(int i=0;i<10;i++){
		if(getToken(GUID_BLACKLIST, ";", i)== guid){
			return false;
		}
		else if(getToken(GUID_BLACKLIST, ";", i)== ""){
			break;
		}
	}

	//If a component want to open (else = close)
	if(goingvisible){
		
		openMainLayoutNow();

		//Resize player bigger on component activity if size is to small
		if(xuiGroup.getheight()<10 && openPlayerFromNow){
			//debugint(getCurAppLeft());//openPlayerFromNow
			normal.resize(getCurAppLeft(),getCurAppTop(),getCurAppWidth(), 400);
		}
		
		if(guid == PL_GUID){ //Playlist
			if(getPublicInt("cpro2.lastDrawer", 1)==DRAWER_PL_ID && open_drawer){
				//do nothing
			}
			else if(mainFrame.getPosition()==0){
				hold_Pl2.setXmlParam("autoopen", "0");
				openTabNo(1);
				hold_Pl2.setXmlParam("autoopen", "1");
			}
		}
		else if(guid == ML_GUID){
			if(active_tab!=0){
				hold_ml.setXmlParam("autoopen", "0");
				openTabNo(0);
				hold_ml.setXmlParam("autoopen", "1");
			}
		}
		else if(guid == VIDEO_GUID){
			//MINI AREA
			if(getPublicInt("cpro2.lastMini", 0) == MINI_VID_ID && area_mini.isVisible()){
				//do nothing
			}
			else if(area_mini.isVisible() && getPublicString("cpro2.defaultview.vid", "MAIN") == "MINI"){
				hold_vid_mini.setXmlParam("autoopen", "0");
				openMini(MINI_VID_ID);
				hold_vid_mini.setXmlParam("autoopen", "1");
			}

			//DRAWER AREA
			else if(getPublicInt("cpro2.lastDrawer", 1) == DRAWER_VID_ID && open_drawer){
				//do nothing
			}
			else if(active_tab!=3 && open_drawer && getPublicString("cpro2.defaultview.vid", "MAIN") == "DRAWER"){
				hold_vid_drawer.setXmlParam("autoopen", "0");
				drawer.sendAction ("switch_to_drawer", "", DRAWER_VID_ID, 0, 0, 0);
				hold_vid_drawer.setXmlParam("autoopen", "1");
			}

			//MAIN AREA
			else if(active_tab!=2){
				hold_vid.setXmlParam("autoopen", "0");
				openTabNo(2);
				hold_vid.setXmlParam("autoopen", "1");
			}
			//debugString("+ drawer =" + integerToString(getPublicInt("cpro2.lastDrawer", 1)), 9);
			//debugString("+ mini   =" + integerToString(getPublicInt("cpro2.lastMini", 1)), 9);

		}
		else if(guid == VIS_GUID){
			//MINI AREA
			if(getPublicInt("cpro2.lastMini", 0) == MINI_VIS_ID && area_mini.isVisible()){
				//do nothing
			}
			else if(area_mini.isVisible() && getPublicString("cpro2.defaultview.vis", "MAIN") == "MINI"){
				hold_avs_mini.setXmlParam("autoopen", "0");
				openMini(MINI_VIS_ID);
				hold_avs_mini.setXmlParam("autoopen", "1");
			}
			
			//DRAWER AREA
			else if(getPublicInt("cpro2.lastDrawer", 1) == DRAWER_VIS_ID && open_drawer){
				//do nothing
			}
			else if(active_tab!=3 && open_drawer && getPublicString("cpro2.defaultview.vis", "MAIN") == "DRAWER"){
				hold_avs_drawer.setXmlParam("autoopen", "0");
				drawer.sendAction ("switch_to_drawer", "", DRAWER_VIS_ID, 0, 0, 0);
				hold_avs_drawer.setXmlParam("autoopen", "1");
			}
			
			//MAIN AREA
			else if(active_tab!=3){
				hold_avs.setXmlParam("autoopen", "0");
				openTabNo(3);
				hold_avs.setXmlParam("autoopen", "1");
			}
		}
		else{
			thirdPartyGuid = guid;
			if(active_tab!=5){
				openTabNo(5);
			}
			else{
				hold_Other.hide();
				hold_Other.setXMLParam("hold", "");
				hold_Other.show();
				hold_Other.setXMLParam("hold", "@all@");
				//hold_Other.setXMLParam("hold", "guid:"+ guid);
				refreshAIOTab.start();
			}
			return false;
		}
		
		if(delayStart) return true; //dont try to open component because its not visible yet!
		return false;
	}
	else{
		closeGUID = guid;
		
		if(delayStart) return false; //somehow if this is here then if you close with vid tab last open it wont timmy on start
		
		openDefaultTab.start();
		return true;
	}
}

refreshAIOTab.onTimer(){
	refreshAIOTab.stop();
	CproTabs.sendAction("update_tabname", hold_Other.getComponentName()+";"+thirdPartyGuid, 5, 0, 0, 0);
}

System.onTitleChange(String newTxt){
	if(!openPlayerFromNow) openPlayerFromNow=true;
}

openDefaultTab.onTimer(){
	openDefaultTab.stop();

	if(closeGUID == PL_GUID){
		if(getPublicInt("cpro2.lastDrawer", 1)==DRAWER_PL_ID && open_drawer){
			drawer.sendAction ("switch_to_drawer", "", 0, 0, 0, 0);
		}
		else if(mainFrame.getPosition()!=0){
			setMainFrame(false);
			return;
		}
	}
	else if(closeGUID == VIDEO_GUID){
		//debugString(closeGUID, 9);
		//debugString("- drawer =" + integerToString(getPublicInt("cpro2.lastDrawer", 1)), 9);
		//debugString("- mini   =" + integerToString(getPublicInt("cpro2.lastMini", 1)), 9);
		if(getPublicInt("cpro2.lastDrawer", 1)==DRAWER_VID_ID){
			drawer.sendAction ("switch_to_drawer", "", 0, 0, 0, 0);
			return;
		}
		else if(getPublicInt("cpro2.lastMini", 0)==MINI_VID_ID){	//video is openned in mini view
			openMini(0);
			return;
		}
	}
	else if(closeGUID == VIS_GUID){
		//if(getPublicInt("cpro2.lastDrawer", 1)==DRAWER_VIS_ID && getPublicString("cpro2.defaultview.vis", "MAIN") != "DRAWER"){
		if(getPublicInt("cpro2.lastDrawer", 1)==DRAWER_VIS_ID){
			drawer.sendAction ("switch_to_drawer", "", 0, 0, 0, 0);
			return;
		}
		//else if(getPublicInt("cpro2.lastMini", 0)==MINI_VIS_ID && getPublicString("cpro2.defaultview.vis", "MAIN") != "MINI"){
		else if(getPublicInt("cpro2.lastMini", 0)==MINI_VIS_ID){
			openMini(0);
			return;
		}
	}

	if(active_tab==0){
		openTabNo(4);//brow
	}
	else{
		openTabNo(0);//ml
	}
}

openWidgetIDS (String ids)
{
	setPublicString("cpro2.lastMainWidgetIDS", ids);
	widgetHolder.setXmlParam("groupid", ids);
}

openTabNo(int tabNo){
	if(delayStart && tabNo!=5){
		delayStartTab=tabNo;
		return;
	}
	else if(delayStart) delayStartTab=tabNo;
	//debugstring("openTabNo()="+integerToString(tabNo),9);

	if(busyWithThisFunction) return;
	busyWithThisFunction=true;
	
	if(skipLoad) wasTabTrig=true;
	active_tab = tabNo;
	
	if(active_tab!=tab_openned){ //check to see if the current tab is already opened...
		tab_library.hide();
		tab_video.hide();
		tab_avs.hide();
		tab_Browser.hide();
		tab_Playlist.hide();
		tab_Other.hide();
		tab_Widget.hide();
	}
	
	tab_openned = tabNo;

	if(tabNo==1){
		if(mainFrame.getPosition()!=0) setMainFrame(false); //if not closed.. close sideview
		if(getPublicInt("cpro2.lastDrawer", 1)==DRAWER_PL_ID && open_drawer)	drawer.sendAction ("switch_to_drawer", "", 0, 0, 0, 0);
		tab_Playlist.show();
	}
	else if(tabNo==2){
		if(getPublicInt("cpro2.lastMini", 0)==MINI_VID_ID){
			openMini(0);
		}
		if(getPublicInt("cpro2.lastDrawer", 1)==DRAWER_VID_ID){
			drawer.sendAction ("switch_to_drawer", "", 0, 0, 0, 0); //close drawer vid
		}
		setPublicString("cpro2.defaultview.vid", "MAIN");
		tab_video.show();
	}
	else if(tabNo==3){
		if(getPublicInt("cpro2.lastMini", 0)==MINI_VIS_ID){
			openMini(0);
		}
		if(getPublicInt("cpro2.lastDrawer", 1)==DRAWER_VIS_ID){
			drawer.sendAction ("switch_to_drawer", "", 0, 0, 0, 0); //close drawer vis
		}
		setPublicString("cpro2.defaultview.vis", "MAIN");
		tab_avs.show();
		checkVisName.start();
	}
	else if(tabNo==4){
		tab_Browser.show();
	}
	else if(tabNo==5){
		tab_Other.show();
		refreshAIOTab.start();
	}
	else if(tabNO==WIDGET_TAB_ID)
	{
		tab_Widget.show();
	}
	else
	{
		tab_library.show();
	}
	
	
	if(tabNo!=-1){
		setPublicInt("cpro2.lastComponentPage", tabNo);
	}
	busyWithThisFunction=false;
	//updateCompStatus();
	
	if (!dontTabCall)
	{
		if (tabNo==WIDGET_TAB_ID)
		{
			CproTabs.sendAction("select_tab", widgetHolder.getXmlParam("groupid"), tabNo, 0, 0, 0);
		}
		else
		{
			CproTabs.sendAction("select_tab", "", tabNo, 0, 0, 0);	
		}
	}
	else dontTabCall=false;
}

checkVisName.onTimer(){
	checkVisName.stop();
	if(tab_openned==3)	visName.setText(hold_avs.getComponentName());
}

openMainLayoutNow(){
	Boolean isShade = (player.getCurLayout() != normal);
	if(isShade)	openMainLayout.start();
}
openMainLayout.onTimer(){
	player.switchToLayout("normal");
	openMainLayout.stop();
}


area_right.onSetVisible(boolean onOff){
	if(onOff && getPublicInt("cpro2.lastComponentPage", 0)==1 && !skipLoad) openTabNo(0); //dont want two playlist hey?
	if(getPublicInt("cpro2.lastDrawer", 1)==DRAWER_PL_ID) drawer.sendAction ("switch_to_drawer", "", 0, 0, 0, 0);
}


area_mini.onResize(int x, int y, int w, int h){
	//Hide/Show the area
	if(w<10){
		area_mini.hide();
		if(getPublicInt("cpro2.lastMini", 0)==4){openMini(0);}
	}
	else if(h<40){
		area_mini.hide();
		if(getPublicInt("cpro2.lastMini", 0)==4){openMini(0);}
	}
	else area_mini.show();
}


but_miniGoto.onleftClick(){
	popMenu = new PopUpMenu;

	popMenu.addCommand("Album Art", 0, 0, 0);
	popMenu.addCommand("File Info", 1, 0, 0);
	popMenu.addCommand("Stored Playlists", 2, 0, 0);
	popMenu.addSeparator();
	popMenu.addCommand("Video", 3, 0, 0);
	popMenu.addCommand("Visualization", 4, 0, 0);
	popMenu.addSeparator();

	int count = 0;
	for (int x = 0; x < dummyBuck.getNumChildren(); x++) {//**
		GuiObject gr = dummyBuck.enumChildren(x);
		popMenu.addCommand(gr.getXMLparam("name"), 100+x, getPublicInt("cpro2.lastMini", 0) == 100+x, 0);
		count++;
	}

	if (count == 0) popMenu.addCommand("No widgets found for this view!", -1, 0, 1);

	popMenu.addSeparator();
	popMenu.addCommand("Widgets Manager", -3, getContainer("widgets.manager").getLayout("normal").isvisible(), 0);

	popMenu.checkCommand(getPublicInt("cpro2.lastMini", 0), 1);

	menuOpen=true;
	int result = popMenu.popAtXY(clientToScreenX(but_miniGoto.getLeft()), clientToScreenY(but_miniGoto.getTop() + but_miniGoto.getHeight()));
	menuOpen = false;
	
	if(result <= 0 && !but_miniGoto.isMouseOverRect() && !wasabiCover.isMouseOverRect()) wasabiCover.onLeaveArea();

	if(result>=0){
		openMini(result);
	}
	else if(result == -3){
		if (getContainer("widgets.manager").getLayout("normal").isvisible())
		{
			normal.sendAction("widget_manager_hide", "", 0,0,0,0);
		}
		else
		{
			normal.sendAction("widget_manager_show", "", 0,0,0,0);
		}
	}
	
	delete popMenu;
	complete;
}

openMini(int miniNo){
	//Safety check to see if the widgets is still there ;)
	if(miniNo>=100){
		if(dummyBuck.getNumChildren()<miniNo-99) miniNo=0;
	}
	int bg = 0;

	mini_Cover.hide();
	mini_Video.hide();
	mini_AVS.hide();
	mini_SavedPL.hide();
	mini_TagView.hide();
	customObj.hide();
  
	setPublicInt("cpro2.lastMini", miniNo);
	if(miniNo!=0){
		wasabiCover.onEnterArea();
	}
	else if(!but_miniGoto.isMouseOverRect() && !wasabiCover.isMouseOverRect()) wasabiCover.onLeaveArea();
  
	if(miniNo==0){
		mini_Cover.show();
	}
	else if(miniNo == MINI_VID_ID){
		setPublicString("cpro2.defaultview.vid", "MINI");

		if(getPublicInt("cpro2.lastComponentPage", 0)==2){
			openTabNo(0);
			openMiniDelay.start();
		}
		else if(getPublicInt("cpro2.lastDrawer", 1)==DRAWER_VID_ID){
				drawer.sendAction ("switch_to_drawer", "", 0, 0, 0, 0); //close drawer vid
				openMiniDelay.start();
		}
		else mini_Video.show();
	}
	else if(miniNo == MINI_VIS_ID){
		setPublicString("cpro2.defaultview.vis", "MINI");

		if(getPublicInt("cpro2.lastComponentPage", 0)==3){
			openTabNo(0);
			//mini_AVS.show();
			openMiniDelay.start();
		}
		else if(getPublicInt("cpro2.lastDrawer", 1)==DRAWER_VIS_ID){
			drawer.sendAction ("switch_to_drawer", "", 0, 0, 0, 0); //close drawer vis
			//mini_AVS.show();
			openMiniDelay.start();
		}
		else mini_AVS.show();

		//hold_avs_mini.setXmlParam("autoopen", "1");
		//debug("WAAA");
		
		//openMiniDelay.start();
	}
	else if(miniNo==2){
		mini_SavedPL.show();
	}
	else if(miniNo==1){
		drawer.sendAction ("release", "TAG", 0, 0, 0, 0);
		mini_TagView.show();
		bg=1;
	}
	else if(miniNo >= 100) {
		GuiObject gr = dummyBuck.enumChildren(miniNo-100);
		String id = getToken(gr.getXmlParam("userdata"), ";", 0);
		
		// @pjn - add background change here! V1.1
		bg = stringToInteger(getToken(gr.getXmlParam("userdata"), ";", 1));
		
		customObj.setXmlParam("groupid", id);
		customObj.show();
	}
	//setMiniBG(bg);
}

openMiniDelay.onTimer(){
	openMiniDelay.stop();
	int miniNo = getPublicInt("cpro2.lastMini", 0);
	
	if(miniNo == MINI_VID_ID){
		mini_Video.show();
	}
	else if(miniNo == MINI_VIS_ID){
		mini_AVS.show();
	}
}

gotoPrevMini(){ //wheelup
	int pos = getPublicInt("cpro2.lastMini", 0);

	if (pos == 0)
	{
		if(dummyBuck.getNumChildren()==0) pos = 2;
		else pos = 99+dummyBuck.getNumChildren();
	}
	else if (pos == 100)
	{
		pos = 2;
	}
	else if(pos>=3 && pos<=4) pos = 2;
	else pos--;

	openMini(pos);
}
gotoNextMini(){ //wheelDown
	int pos = getPublicInt("cpro2.lastMini", 0);

	if(pos>=3 && pos<=4) pos = 2;

	if (pos == 2)
	{
		if(dummyBuck.getNumChildren()==0) pos = 0;
		else pos = 100;
	}
	else if (pos == 99+dummyBuck.getNumChildren())
	{
		pos = 0;
	}
	else pos++;

	openMini(pos);
}

normal.onMouseWheelUp(int clicked , int lines){
	if(mouse_but_miniGoto){
		gotoPrevMini();
		complete;
		return 1;
	}
}
normal.onMouseWheelDown(int clicked , int lines){
	if(mouse_but_miniGoto){
		gotoNextMini();
		complete;
		return 1;
	}
}

/*setMiniBG(int mode){
	if(mode==0){
		gadgrid1.show();
		gadgrid1a.hide();
		gadgrid1a_overlay.hide();
		
		gadgrid2.setXmlParam("x", "6");
		gadgrid2.setXmlParam("w", "-6");
	}
	else if(mode==1){
		if(getPublicInt("cPro.transparentsave", 0)){
			gadgrid1.hide();
			gadgrid1a.show();
			gadgrid1a_overlay.show();

			gadgrid2.setXmlParam("x", "247");
			gadgrid2.setXmlParam("w", "-247");
		}
		else{
			gadgrid1.show();
			gadgrid1a.show();
			gadgrid1a_overlay.show();
			
			gadgrid2.setXmlParam("x", "6");
			gadgrid2.setXmlParam("w", "-6");
		}
	}
	else{
		debug("Error: Mini background not found!");
	}
}*/

refreshComponentButtons(){
	if(getPublicInt("cpro2.One.TabStatus.1", 1)){
			guihold_Pl2.setXmlParam("h", "-19");
			tabbut_pl.show();
	}
	else{
			guihold_Pl2.setXmlParam("h", "0");
			tabbut_pl.hide();
	}

	if(getPublicInt("cpro2.One.TabStatus.2", 1)){
			hold_vid.setXmlParam("h", "-19");
			tabbut_vid.show();
	}
	else{
			hold_vid.setXmlParam("h", "0");
			tabbut_vid.hide();
	}
	
	if(getPublicInt("cpro2.One.TabStatus.3", 1)){
			hold_avs.setXmlParam("h", "-19");
			visRectBg.setXmlParam("h", "-19");
			tabbut_avs.show();
	}
	else{
			hold_avs.setXmlParam("h", "0");
			visRectBg.setXmlParam("h", "0");
			tabbut_avs.hide();
	}
	//updateCompStatus();
}

/*
updateCompStatus(){
	if(tab_openned==0) setCompStatus(false);
	else if(tab_openned==4) setCompStatus(false);
	else if(tab_openned==5) setCompStatus(false);
	else if(tab_openned==WIDGET_TAB_ID) setCompStatus(widgetStatus);
	else setCompStatus(getPublicInt("Cpro.One.TabStatus."+integerToString(tab_openned), 1));
}

setCompStatus(boolean onOff){
	if(cuseqbg){
		if(onOff){
			compGrid.setXmlParam("bottomleft", "player.suiframe.7");
			compGrid.setXmlParam("bottom", "player.suiframe.8");
			compGrid.setXmlParam("bottomright", "player.suiframe.9");
		}
		else{
			compGrid.setXmlParam("bottomleft", "player.suiframe.7.alt");
			compGrid.setXmlParam("bottom", "player.suiframe.8.alt");
			compGrid.setXmlParam("bottomright", "player.suiframe.9.alt");
		}
	}
}
*/


xuiGroup.onAction (String action, String param, int x, int y, int p1, int p2, GuiObject source){
	if (strlower(action) == "switch_to_tab"){ //used by other script to show tab holder & select tab
		if (x == WIDGET_TAB_ID){
			openWidgetIDS(param);
		}
		openTabNo(x);
	}
	else if (strlower(action) == "show_tab"){ //used by the tabs to show tab holder
		dontTabCall=true;
		if (x == WIDGET_TAB_ID)
			openWidgetIDS(param);
		openTabNo(x);
	}
	else if (strlower(action) == "refresh_tab_status"){ //used by the tabs to show tab holder
		refreshComponentButtons();
	}
	else if (strlower(action) == "browser_url"){
		openTabNo(4);
		xuiBrowser.sendAction ("openurl", param, 0, 0, 0, 0);
	}
	else if (strlower(action) == "browser_search"){
		openTabNo(4);
		xuiBrowser.sendAction ("search", param, 0, 0, 0, 0);
	}
	else if (strlower(action) == "drawer_onoff"){
		setDrawer(x);		
	}
	else if (strlower(action) == "switch_to_mini"){
		openMini(x);		
	}
	else if (strlower(action) == "frame_main_setsize"){
		mainFrame.setPosition(x);
	}
	else if (strlower(action) == "frame_mini_setsize"){
		setPublicInt("cpro2.e1.frame2", plFrame.getPosition());
		setFrame2(x, area_right_pl.getHeight());
	}
	else if (strlower(action) == "sideview_onoff"){
		if(x==1) setMainFrame(true);
		else setMainFrame(false);
	}
	else if (strlower(action) == "release"){
		if(param=="VIS"){
			if(getPublicInt("cpro2.lastMini", 0)==4) openMini(0);
			if(getPublicInt("cpro2.lastComponentPage", 0)==3) openTabNo(0);
		}
		else if(param=="PL"){
			if(getPublicInt("cpro2.lastComponentPage", 0)==1) openTabNo(0);
			if(mainFrame.getPosition()!=0) setMainFrame(false);
		}
		else if(param=="VID"){
			if(getPublicInt("cpro2.lastMini", 0)==3) openMini(0);
			if(getPublicInt("cpro2.lastComponentPage", 0)==2) openTabNo(0);
		}
		else if(param=="TAG"){
			if(getPublicInt("cpro2.lastMini", 0)==1) openMini(0);
		}
	}
	else if(strlower(action) == "widget_statusbar"){
		widgetStatus=x;
		//updateCompStatus();
	}
	else if(strlower(action) == "refresh_drawer_h"){
		setDrawerSizeSave(getPublicInt("cpro2.drawer.h", -119));
		setDrawer(getPublicInt("cpro2.draweropened", 1));
	}
}

//Main Frame code
xuiGroup.onResize(int x, int y, int w, int h){
	if(_powerSave4!=h){
		setDrawer(getPublicInt("cpro2.draweropened", 1));
		_powerSave4=h;
	}
	
	if(w<413){
		if(_powerSave2!=1){
			ocFrame.hide();
			tog_drawer.setXmlParam("x", "-24");
			CproTabs.setXmlParam("w", "-27");

			if(mainFrame.getPosition()!=0){
				mainFrame.setPosition(0);
				setPublicInt("cpro2.mainframe.sysclose", 1);
			}
			_powerSave2=1;
		}
	}
	else{
		if(_powerSave2!=2){
			tog_drawer.setXmlParam("x", "-48");
			ocFrame.show();
			CproTabs.setXmlParam("w", "-51");

			_powerSave2=2;
		}
	}
	if(w>=413 && getPublicInt("cpro2.mainframe.sysclose", 0)==1){
		setMainFrame(true);
	}

}
area_left.onResize(int x, int y, int w, int h){
	setFrame1();
}

area_right.onResize(int x, int y, int w, int h){
	if(stopResizeRight){
		stopResizeRight = false;
		return;
	}
	stopResizeRight = true;

	if(w<10){
		if(_powerSave3!=1){
			area_right.hide();
			closeFrame.hide();
			openFrame.show();
			if(!mouseDownF1){
				mainFrame.setXmlParam("resizable", "0");
				mainFrame.setXmlParam("maxwidth", "-224");
			}

			_powerSave3=1;
		}
	}
	else{
		if(_powerSave3!=2){
			area_right.show();
			closeFrame.show();
			openFrame.hide();

			_powerSave3=2;
		}
	}
	
	if(mainFrame.getPosition()!=0){
		if(w<193){
			plText1.hide();
			gridPL.hide();
			plHideSearch.hide();
			plHideSearchIcon.hide();
		}
		else if(w<208){
			plText1.hide();
			gridPL.hide();
			plHideSearch.show();
			plHideSearchIcon.show();
		}
		else{
			plText1.show();
			gridPL.show();
			plHideSearch.show();
			plHideSearchIcon.show();
		}
		
		if(w<141 && mainFrame.getPosition()!=0){
			if(mouseDownF1){
				setPublicInt("cpro2.mainframe.sysclose", 0);
				mainFrame.setPosition(0);
			}
		}
	}
	
	setFrame1();

	stopResizeRight = false;
}


mainFrame.onLeftButtonDown(int x, int y){
	setPublicInt("cpro2.e1.closeframe.lastpos", mainFrame.getPosition()); //TESTING THIS...CHANGE BACK
	mainFrame.setXmlParam("resizable", "1");
	mainFrame.setXmlParam("maxwidth", "-232");
	mouseDownF1=true;
}

mainFrame.onLeftButtonUp(int x, int y){
	mouseDownF1=false;
	if(area_right.getWidth()<10){
		mainFrame.setXmlParam("resizable", "0");
		mainFrame.setXmlParam("maxwidth", "-224");
	}
	else setPublicInt("cpro2.e1.closeframe.lastpos", mainFrame.getPosition()); //TESTING THIS...CHANGE BACK
	
	mainFrame.setPosition(mainFrame.getPosition()); //This is done to refresh the hide of the resizer ;)
}

setFrame1(){
	if(mainFrame.getPosition()<181 && mainFrame.getPosition()>=141){
		mainFrame.setPosition(161);
	}
}
closeFrame.onLeftClick(){
	setMainFrame(false);
}
openFrame.onLeftClick(){
	setMainFrame(true);
}

setMainFrame(boolean open){
	setPublicInt("cpro2.mainframe.sysclose", 0);
	if(open){
		int pos = getPublicInt("cpro2.e1.closeframe.lastpos", 200);
		if(pos<161) pos = 161;
		mainFrame.setXmlParam("resizable", "1");
		mainFrame.setXmlParam("maxwidth", "-232");
		mainFrame.setPosition(pos);
	}
	else{
		setPublicInt("cpro2.e1.closeframe.lastpos", mainFrame.getPosition());
		mainFrame.setXmlParam("resizable", "0");
		mainFrame.setXmlParam("maxwidth", "-224");
		mainFrame.setPosition(0);
	}
	mainFrame.setPosition(mainFrame.getPosition()); //This is done to refresh the hide of the resizer ;)
}


//Mini Frame code
plFrame.onLeftButtonDown(int x, int y){
	mouseDownF2=true;
}
plFrame.onLeftButtonUp(int x, int y){
	mouseDownF2=false;
	setPublicInt("cpro2.e1.frame2", plFrame.getPosition());

}
area_right_pl.onResize(int x, int y, int w, int h){
	if(mouseDownF2)	setPublicInt("cpro2.e1.frame2", plFrame.getPosition());
	setFrame2(getPublicInt("cpro2.e1.frame2", 200), h);
}
setFrame2(int pos, int h){
	int output = pos;

	//Magnet-clip the framedivider
	if(pos<79){
		output=1;
	}
	else{
		output=xuiGroup.getHeight()-48;
	}
	
	if(output<79) output=1;

	if(output>getPublicInt("cpro2.e1.frame2", 200)) output=getPublicInt("cpro2.e1.frame2", 200);
	
	if(output!=plFrame.getPosition()) plFrame.setPosition(output);
}

plText1.onLeftButtonDblClk(int x, int y){
	PlEdit.showCurrentlyPlayingTrack ();
}
plText2.onLeftButtonDblClk(int x, int y){
	PlEdit.showCurrentlyPlayingTrack ();
}

ssWinHol.onTimer(){
	delayStart=false;
	ssWinHol.stop();
	openTabNo(delayStartTab);
}









setDrawer(boolean onOff){
	busyWithDrawer=true;
	boolean dontSave = false;

	//if(xuiGroup.getHeight()<DEF_DRAWER_H+90){ //0 = first load... not drawn yet
	if(xuiGroup.getHeight()<drawer.getHeight()+90){ //0 = first load... not drawn yet
		onOff=false;
		dontSave=true;
	}
	if(onOff){
		drawer.show();
		drawerResize.show();
		//mainTabsheet.setXmlParam("h", integerToString(DEF_DRAWER_H*-1-4));
		setDrawerSizeSave(getPublicInt("cpro2.drawer.h", -119));
		tog_drawer.setXmlParam("tooltip", "Close drawer");
		open_drawer=true;
	}
	else{
		drawer.hide();
		drawerResize.hide();
		mainTabsheet.setXmlParam("h", "0");
		_powerSave1=0;
		open_drawer=false;
		tog_drawer.setXmlParam("tooltip", "Open drawer");
	}
	tog_drawer.setActivated(open_drawer);

	if(!dontSave){
		setPublicInt("cpro2.draweropened", open_drawer);
	}
	busyWithDrawer=false;
}
tog_drawer.onToggle(Boolean onoff){
	setDrawer(onoff);
}

drawer.onSetVisible(boolean onOff){
	if(!busyWithDrawer) setDrawer(getPublicInt("cpro2.draweropened", 1));
}





drawerResize.onLeftButtonDown(int x, int y){
	dmr_down=true;
}
drawerResize.onLeftButtonUp(int x, int y){
	dmr_down=false;
}
drawerResize.onMouseMove(int x, int y){
	if(dmr_down){
		int t = y-xuiGroup.getHeight()-xuiGroup.getTop();
		if(t<-xuiGroup.getHeight()+120) t=-xuiGroup.getHeight()+120;
		
		//if(getPublicInt("cpro2.lastDrawer", 0)==0) return;
		setDrawerSizeSave(t-t%10);
	}
}

setDrawerSizeSave(int h){
	//debugint(h);
	if(_powerSave1!=h){
		_powerSave1 = h;
		//these changes will be saved
		if(h>-119) h=-119;
		setPublicInt("cpro2.drawer.h", h);
		

		//these changes will not be saved
		//if(getPublicInt("cpro2.lastDrawer", 0)==0) h=-119;

		mainTabsheet.setXmlParam("h", integerToString(h-4));
		drawer.setXmlParam("y", integerToString(h));
		drawer.setXmlParam("h", integerToString(-h));
		drawerResize.setXmlParam("y", integerToString(h-4));
	}
}

area_mini.onAction (String action, String param, Int x, int y, int p1, int p2, GuiObject source)
{
	if (strlower(action) == "show_widget")
	{
		for (int i = 0; i < dummyBuck.getNumChildren(); i++)
		{
			GuiObject d = dummyBuck.enumChildren(i);
			if (getToken(d.getXmlParam("userdata"), ";",0) == param)
			{
				normal.getContainer().switchToLayout("normal");

				boolean resize = false;
				int w = normal.getWidth();
				int h = normal.getHeight();

				if (normal.getHeight() < 238)
				{
					resize = true;
					h = 388;
				}
				if (normal.getWidth() < 427)
				{
					resize = true;
					w = 427;
				}
				if (resize)
				{
					setPublicInt("cpro2.h",h);
					setPublicInt("cpro2.w",w);
					normal.resize(normal.getLeft(), normal.getTop(), w, h);	
				}
				
				setMainFrame(true);

				if (plFrame.getPosition() < 40)
				{
					setPublicInt("cpro2.e1.frame2", 160);
					setFrame2(160,0);
				}

				openMini(i+100);	
			}
		}	
	}
}

tab_Playlist.onSetVisible(boolean onOff){ //Big Screen PL
	if(onOff){
		tog_search = tab_Playlist.getObject("centro2.group.pl.buttons").findObject("pl.search.toggle"); //Main Screen
		playlist_search_attib.onDataChanged();
		//tog_search.setActivated(getPublicInt("Cpro2.plsearch", 1));
	}
}
area_right_pl.onSetVisible(boolean onOff){ //Right Side PL
	if(onOff){
		tog_search = area_right_pl.getObject("centro2.group.pl.buttons").findObject("pl.search.toggle"); //Regs PL
		playlist_search_attib.onDataChanged();
		//tog_search.setActivated(getPublicInt("Cpro2.plsearch", 1));
	}
}
drawer_pl.onSetVisible(boolean onOff){ //Right Side PL
	if(onOff){
		tog_search = drawer_pl.getObject("centro2.group.pl.buttons").findObject("pl.search.toggle"); //Drawer PL
		playlist_search_attib.onDataChanged();
		//tog_search.setActivated(getPublicInt("Cpro2.plsearch", 1));
	}
}
playlist_search_attib.onDataChanged(){
	tog_search.setActivated(stringToInteger(getData()));
	/*if (getData() == "0"){
		//debug("af");
		//tog_search.setActivated(false);
	}
	else{
		//tog_search.setActivated(true);
	}*/
}


tog_search.onToggle(boolean onOff){
	//System.setPublicInt("Cpro2.plsearch", onOff);
		//debug("toggle " + integerToString(onOff));
	playlist_search_attib.setData(integerToString(onOff));
	//debug("toggle " + integerToString(onOff));

}


Function cProLoaded();

/* Notify Widget Manager, what cool widgets has been installed */ 
StartupCallback.onLoaded()
{
	cProLoaded();
}

/** somehow winamp creates PVC calls when i do this stuff above */
cProLoaded ()
{
	int widgetPlace = normal.onAction("widget_manager_register", "Side Area", 0,0,0,0,area_mini); // TODO Translate
	for (int i = 0; i < dummyBuck.getNumChildren(); i++)
	{
		GuiObject d = dummyBuck.enumChildren(i);
		normal.onAction("widget_manager_check", getToken(d.getXmlParam("userdata"), ";", 0), widgetPlace,0,0,0,d);
	}
	widgetPlace = normal.onAction("widget_manager_done", "", widgetPlace,0,0,0,area_mini);
}

// ADD VIS OPTION IN MAIN MENU!
sui_vis_attrib.onDataChanged(){
	if(myChange){
		myChange=false;
		return;
	}

	if(sui_vis_attrib.getData()=="1")
	{
		//MINI AREA
		if(area_mini.isVisible() && getPublicString("cpro2.defaultview.vis", "MAIN") == "MINI"){
			openMini(MINI_VIS_ID);
		}
		
		//DRAWER AREA
		else if(active_tab!=3 && open_drawer && getPublicString("cpro2.defaultview.vis", "MAIN") == "DRAWER"){
			drawer.sendAction ("switch_to_drawer", "", DRAWER_VIS_ID, 0, 0, 0);
		}
		
		//MAIN AREA
		else if(active_tab!=3){
			openTabNo(3);
		}
	}
	else if(sui_vis_attrib.getData()=="0"){
		//MINI AREA
		if(getPublicString("cpro2.defaultview.vis", "MAIN") == "MINI"){
			openMini(0);
		}
		
		//DRAWER AREA
		else if(getPublicString("cpro2.defaultview.vis", "MAIN") == "DRAWER"){
			drawer.sendAction ("switch_to_drawer", "", 0, 0, 0, 0);
		}
		
		//MAIN AREA
		else if(active_tab==3){
			openTabNo(0);
		}
	}
}

mini_AVS.onSetVisible(boolean onOff){
	myChange=true;
	if(onOff) sui_vis_attrib.setData("1");
	else sui_vis_attrib.setData("0");
}

tab_avs.onSetVisible(boolean onOff){
	myChange=true;
	if(onOff) sui_vis_attrib.setData("1");
	else sui_vis_attrib.setData("0");
}

hold_avs_drawer.onSetVisible(boolean onOff){
	myChange=true;
	if(onOff) sui_vis_attrib.setData("1");
	else sui_vis_attrib.setData("0");
}
